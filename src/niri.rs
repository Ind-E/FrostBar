use color_eyre::eyre::bail;
use iced::{
    Background, Border, Color, Element, Length, Theme,
    advanced::subscription,
    alignment::Horizontal,
    border::Radius,
    futures::Stream,
    padding::top,
    widget::{
        Column, Container, MouseArea, Svg,
        container::{self, StyleFn},
        svg, text,
    },
};
use itertools::Itertools;
use niri_ipc::{Event, Request, socket::Socket};
use std::{cmp::Ordering, collections::HashMap, hash::Hash, pin::Pin, sync::Arc};
use tokio::sync::{Mutex, mpsc};

use crate::{Message, MouseEnterEvent};

pub struct NiriEvents;

#[derive(PartialEq, Eq)]
pub struct Window<'a> {
    pub title: &'a Option<String>,
    pub id: &'a u64,
    pub icon: Option<svg::Handle>,
}

impl<'a> Window<'a> {
    pub fn to_widget(&self) -> Element<'a, Message> {
        if let Some(icon) = &self.icon {
            MouseArea::new(Svg::new(icon.clone()).height(24).width(24)).into()
        } else {
            MouseArea::new(text(self.id)).into()
        }
    }
}

impl<'a> PartialOrd for Window<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> Ord for Window<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

pub struct Workspace<'a> {
    pub output: &'a Option<String>,
    pub idx: &'a u8,
    pub is_active: &'a bool,
    pub windows: Vec<Window<'a>>,
}

impl<'a> Workspace<'a> {
    pub fn to_widget(&self, hovered: bool) -> Element<'a, Message> {
        MouseArea::new(
            Container::new(
                self.windows.iter().sorted().fold(
                    Column::new()
                        .align_x(Horizontal::Center)
                        .spacing(5)
                        .push(text(self.idx - 1).size(20)),
                    |col, w| col.push(w.to_widget()),
                ),
            )
            .style(workspace_style(self.is_active, hovered))
            .padding(top(4).bottom(4))
            .width(Length::Fill)
            .align_x(Horizontal::Center),
        )
        .on_press(Message::WorkspaceClicked(*self.idx))
        .on_enter(Message::MouseEntered(MouseEnterEvent::Workspace(*self.idx)))
        .on_exit(Message::MouseExited(MouseEnterEvent::Workspace(*self.idx)))
        .into()
    }
}

fn workspace_style<'a>(active: &'a bool, hovered: bool) -> StyleFn<'a, Theme> {
    Box::new(move |_| container::Style {
        border: Border {
            color: if *active {
                Color::WHITE
            } else {
                Color::from_rgb(0.3, 0.3, 0.3)
            },
            width: 2.0,
            radius: Radius::new(12),
        },
        background: Some(Background::Color(if hovered {
            Color::from_rgba(0.8, 0.8, 0.8, 0.02)
        } else {
            Color::TRANSPARENT
        })),
        ..Default::default()
    })
}

#[derive(Default)]
pub struct NiriState {
    pub workspaces: HashMap<u64, niri_ipc::Workspace>,
    pub windows: HashMap<u64, niri_ipc::Window>,
}

impl NiriState {
    pub fn on_event(&mut self, event: Event) {
        match event {
            Event::WorkspacesChanged { workspaces } => {
                self.workspaces = workspaces.into_iter().map(|ws| (ws.id, ws)).collect()
            }
            Event::WindowsChanged { windows } => {
                self.windows = windows.into_iter().map(|w| (w.id, w)).collect()
            }
            Event::WindowOpenedOrChanged { window } => {
                self.windows.insert(window.id, window);
            }
            Event::WindowClosed { id } => {
                self.windows.remove(&id);
            }
            Event::WorkspaceActivated { id, focused } => {
                let output = self.workspaces.iter().find_map(|(wid, ws)| {
                    if wid == &id { ws.output.clone() } else { None }
                });
                for (_, ws) in &mut self.workspaces {
                    if ws.output == output {
                        ws.is_active = false;
                    }
                }
                self.workspaces.get_mut(&id).map(|ws| {
                    ws.is_active = true;
                    ws.is_focused = focused;
                });
            }
            Event::WorkspaceUrgencyChanged { id: _, urgent: _ } => {}
            Event::WorkspaceActiveWindowChanged {
                workspace_id: _,
                active_window_id: _,
            } => {}
            Event::WindowFocusChanged { id: _ } => {}
            Event::WindowUrgencyChanged { id: _, urgent: _ } => {}
            Event::KeyboardLayoutsChanged {
                keyboard_layouts: _,
            } => {}
            Event::KeyboardLayoutSwitched { idx: _ } => {}
            Event::OverviewOpenedOrClosed { is_open: _ } => {}
        }
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum IpcError {
    #[error("Failed to connect to niri socket: {0}")]
    ConnectionFailed(String),
    #[error("Niri responded unexpectedly: {0:?}")]
    UnexpectedResponse(niri_ipc::Response),
    #[error("IPC read error: {0}")]
    ReadError(String),
}

impl subscription::Recipe for NiriEvents {
    type Output = Result<Event, IpcError>;

    fn hash(&self, state: &mut subscription::Hasher) {
        std::any::TypeId::of::<Self>().hash(state);
    }

    fn stream(
        self: Box<Self>,
        _input: subscription::EventStream,
    ) -> Pin<
        Box<
            (
                dyn Stream<Item = std::result::Result<niri_ipc::Event, IpcError>>
                    + std::marker::Send
                    + 'static
            ),
        >,
    > {
        Box::pin(async_stream::stream! {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

            tokio::task::spawn_blocking(move || {
                if let Err(e) = run_niri_listener(tx) {
                    eprintln!("Niri IPC listener failed: {}", e);
                }
            });

            while let Some(event_result) = rx.recv().await {
                yield event_result;
            }
        })
    }
}

fn run_niri_listener(
    tx: tokio::sync::mpsc::UnboundedSender<Result<Event, IpcError>>,
) -> color_eyre::Result<()> {
    let mut sock = match niri_ipc::socket::Socket::connect() {
        Ok(s) => s,
        Err(e) => {
            let _ = tx.send(Err(IpcError::ConnectionFailed(e.to_string())));
            bail!("Failed to connect to socket: {e}");
        }
    };

    match sock.send(Request::EventStream)? {
        Ok(niri_ipc::Response::Handled) => {}
        Ok(other) => {
            let _ = tx.send(Err(IpcError::UnexpectedResponse(other.clone())));
            bail!("Niri responded unexpectedly {other:?}");
        }
        Err(e) => {
            let _ = tx.send(Err(IpcError::ConnectionFailed(e.to_string())));
            bail!("Niri handshake failed: {e}");
        }
    }

    eprintln!("Successfully subscribed to Niri event stream.");
    let mut read_event = sock.read_events();

    loop {
        match read_event() {
            Ok(event) => {
                if tx.send(Ok(event)).is_err() {
                    break;
                }
            }
            Err(e) => {
                let _ = tx.send(Err(IpcError::ReadError(e.to_string())));
                bail!("Failed to read event: {e}");
            }
        }
    }

    Ok(())
}

pub async fn run_niri_request_handler(
    mut request_rx: mpsc::Receiver<Request>,
    socket: Arc<Mutex<Socket>>,
) {
    while let Some(request) = request_rx.recv().await {
        let mut sock = socket.lock().await;
        if let Err(e) = sock.send(request) {
            eprintln!("Failed to send request to niri: {}", e);
        }
    }
}
