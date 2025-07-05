use color_eyre::eyre::bail;
use iced::{
    Background, Border, Color, Element, Length, Theme,
    advanced::subscription,
    alignment::Horizontal,
    border::Radius,
    futures::Stream,
    mouse::Interaction,
    padding::top,
    widget::{
        Column, Container, Image, MouseArea, Svg,
        container::{self, StyleFn},
        text,
    },
};
use itertools::Itertools;
use niri_ipc::{Event, Request, socket::Socket};
use std::{cmp::Ordering, collections::HashMap, hash::Hash, pin::Pin, sync::Arc};
use tokio::sync::{Mutex as TokioMutex, mpsc};

use crate::{
    bar::{Message, MouseEvent},
    icon_cache::{Icon, IconCache},
};

pub struct NiriEvents;

#[derive(Eq)]
pub struct Window {
    pub title: Option<String>,
    pub id: u64,
    pub icon: Option<Icon>,
}

impl PartialEq for Window {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<'a> Window {
    pub fn to_widget(&self) -> Element<'a, Message> {
        match &self.icon {
            Some(Icon::Svg(handle)) => {
                MouseArea::new(Svg::new(handle.clone()).height(24).width(24))
                    .on_right_press(Message::FocusWindow(self.id))
                    .into()
            }
            Some(Icon::Raster(handle)) => {
                MouseArea::new(Image::new(handle.clone()).height(24).width(24))
                    .on_right_press(Message::FocusWindow(self.id))
                    .into()
            }
            Option::None => unreachable!(),
        }
    }
}

impl PartialOrd for Window {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Window {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

pub struct Workspace {
    pub output: Option<String>,
    pub idx: u8,
    pub id: u64,
    pub is_active: bool,
    pub windows: HashMap<u64, Window>,
}

impl<'a> Workspace {
    pub fn to_widget(
        &self,
        hovered: bool,
        id: container::Id,
    ) -> Element<'a, Message> {
        Container::new(
            MouseArea::new(
                Container::new(
                    self.windows.iter().sorted().fold(
                        Column::new()
                            .align_x(Horizontal::Center)
                            .spacing(5)
                            .push(text(self.idx - 1).size(20)),
                        |col, (_, w)| col.push(w.to_widget()),
                    ),
                )
                .style(workspace_style(self.is_active, hovered))
                .padding(top(5).bottom(5))
                .width(Length::Fill)
                .align_x(Horizontal::Center),
            )
            .on_press(Message::FocusWorkspace(self.idx))
            .on_enter(Message::MouseEntered(MouseEvent::Workspace(self.idx)))
            .on_exit(Message::MouseExited(MouseEvent::Workspace(self.idx)))
            .interaction(Interaction::Pointer),
        )
        .id(id)
        .into()
    }

    pub fn window_titles(&self) -> String {
        let window_titles: Vec<String> = self
            .windows
            .iter()
            .sorted()
            .map(|(_, w)| {
                if let Some(title) = &w.title {
                    title.clone()
                } else {
                    "N/A".to_string()
                }
            })
            .filter(|t| !t.is_empty())
            .collect();

        return window_titles.join("\n");
    }
}

fn workspace_style<'a>(active: bool, hovered: bool) -> StyleFn<'a, Theme> {
    Box::new(move |_| container::Style {
        border: Border {
            color: if active {
                Color::WHITE
            } else {
                Color::from_rgb(0.3, 0.3, 0.3)
            },
            width: 2.0,
            radius: Radius::new(12),
        },
        background: Some(Background::Color(if hovered {
            Color::from_rgba(0.8, 0.8, 0.8, 0.015)
        } else {
            Color::TRANSPARENT
        })),
        ..Default::default()
    })
}

pub struct NiriState {
    pub workspaces: HashMap<u64, Workspace>,
    pub windows: HashMap<u64, niri_ipc::Window>,
    pub icon_cache: IconCache,
}

fn map_window(window: &niri_ipc::Window, icon_cache: &mut IconCache) -> Window {
    Window {
        title: window.title.clone(),
        id: window.id,
        icon: window
            .app_id
            .as_ref()
            .and_then(|app_id| icon_cache.get_icon(app_id).clone()),
    }
}

impl NiriState {
    pub fn new(icon_cache: IconCache) -> Self {
        Self {
            workspaces: HashMap::new(),
            windows: HashMap::new(),
            icon_cache,
        }
    }

    pub fn on_event(&mut self, event: Event) {
        match event {
            Event::WorkspacesChanged { workspaces } => {
                self.workspaces = workspaces
                    .into_iter()
                    .map(|ws| Workspace {
                        output: ws.output,
                        idx: ws.idx,
                        id: ws.id,
                        is_active: ws.is_active,
                        windows: self
                            .windows
                            .iter()
                            .filter(|(_, w)| w.workspace_id == Some(ws.id))
                            .map(|(id, w)| {
                                (*id, map_window(w, &mut self.icon_cache))
                            })
                            .collect(),
                    })
                    .map(|ws| (ws.id, ws))
                    .collect()
            }
            Event::WindowsChanged { windows } => {
                self.windows = windows.into_iter().map(|w| (w.id, w)).collect();

                self.workspaces.values_mut().for_each(|ws| {
                    ws.windows = self
                        .windows
                        .values()
                        .filter(|w| w.workspace_id == Some(ws.id))
                        .map(|w| (w.id, map_window(&w, &mut self.icon_cache)))
                        .collect()
                });
            }
            Event::WindowOpenedOrChanged { window } => {
                let id = window.id.clone();
                self.windows.insert(window.id, window);
                let window = self.windows.get(&id).unwrap();

                if let Some(ws_id) = window.workspace_id
                    && let Some(ws) = self.workspaces.get_mut(&ws_id)
                {
                    ws.windows.insert(
                        window.id,
                        map_window(&window, &mut self.icon_cache),
                    );
                }
            }
            Event::WindowClosed { id } => {
                self.windows.remove(&id);
                self.workspaces.values_mut().for_each(|ws| {
                    ws.windows.remove(&id);
                });
            }
            Event::WorkspaceActivated { id, .. } => {
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
    socket: Arc<TokioMutex<Socket>>,
) {
    while let Some(request) = request_rx.recv().await {
        let mut sock = socket.lock().await;
        if let Err(e) = sock.send(request) {
            eprintln!("Failed to send request to niri: {}", e);
        }
    }
}
