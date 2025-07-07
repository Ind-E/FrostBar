use iced::{
    Element, Length,
    advanced::subscription,
    alignment::Horizontal,
    futures::Stream,
    mouse::Interaction,
    padding::top,
    widget::{
        Column, Container, Image, MouseArea, Svg,
        container::{self},
        text,
    },
};
use itertools::Itertools;
use niri_ipc::{Action, Event, Request, WorkspaceReferenceArg, socket::Socket};
use std::{collections::HashMap, hash::Hash, pin::Pin, sync::Arc};
use tokio::sync::mpsc::{self};

use crate::{
    bar::{Message, MouseEvent},
    icon_cache::{Icon, IconCache},
    style::workspace_style,
};

pub struct Window {
    pub id: u64,
    pub icon: Option<Icon>,
}

impl<'a> Window {
    pub fn to_widget(&self, id: Option<container::Id>) -> Element<'a, Message> {
        let icon: Element<'a, Message> = match &self.icon {
            Some(Icon::Svg(handle)) => {
                Svg::new(handle.clone()).height(24).width(24).into()
            }
            Some(Icon::Raster(handle)) => {
                Image::new(handle.clone()).height(24).width(24).into()
            }
            Option::None => unreachable!(),
        };

        let container = Container::new(
            MouseArea::new(icon)
                .on_right_press(Message::NiriAction(Action::FocusWindow {
                    id: self.id,
                }))
                .on_enter(Message::MouseEntered(MouseEvent::Window(self.id)))
                .on_exit(Message::MouseExited(MouseEvent::Window(self.id))),
        );

        if let Some(id) = id {
            container.id(id).into()
        } else {
            container.into()
        }
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
        window_ids: &HashMap<u64, container::Id>,
    ) -> Element<'a, Message> {
        Container::new(
            MouseArea::new(
                Container::new(
                    self.windows
                        .values()
                        .sorted_by(|w1, w2| w1.id.cmp(&w2.id))
                        .fold(
                            Column::new()
                                .align_x(Horizontal::Center)
                                .spacing(5)
                                .push(text(self.idx - 1).size(20)),
                            |col, w| {
                                col.push(
                                    w.to_widget(window_ids.get(&w.id).cloned()),
                                )
                            },
                        ),
                )
                .style(workspace_style(self.is_active, hovered))
                .padding(top(5).bottom(5))
                .width(Length::Fill)
                .align_x(Horizontal::Center),
            )
            .on_press(Message::NiriAction(Action::FocusWorkspace {
                reference: WorkspaceReferenceArg::Id(self.id),
            }))
            .on_enter(Message::MouseEntered(MouseEvent::Workspace(self.id)))
            .on_exit(Message::MouseExited(MouseEvent::Workspace(self.id)))
            .interaction(Interaction::Pointer),
        )
        .into()
    }
}

pub struct NiriState {
    pub workspaces: HashMap<u64, Workspace>,
    pub windows: HashMap<u64, niri_ipc::Window>,
    pub hovered_workspace_id: Option<u64>,
    icon_cache: IconCache,
    sender: Arc<tokio::sync::mpsc::Sender<Request>>,
}

fn map_window(window: &niri_ipc::Window, icon_cache: &mut IconCache) -> Window {
    Window {
        id: window.id,
        icon: window
            .app_id
            .as_ref()
            .and_then(|app_id| icon_cache.get_icon(app_id).clone()),
    }
}

impl NiriState {
    pub fn new(icon_cache: IconCache) -> Self {
        let (request_tx, request_rx) = mpsc::channel(32);
        let request_socket = match niri_ipc::socket::Socket::connect() {
            Ok(sock) => sock,
            Err(e) => panic!("Failed to create niri request socket: {}", e),
        };

        tokio::spawn(run_niri_request_handler(request_rx, request_socket));

        Self {
            workspaces: HashMap::new(),
            windows: HashMap::new(),
            hovered_workspace_id: None,
            icon_cache,
            sender: Arc::new(request_tx),
        }
    }

    pub fn handle_action(&mut self, action: Action) -> iced::Task<Message> {
        let request = Request::Action(action);
        let sender = self.sender.clone();
        iced::Task::perform(async move { sender.send(request).await }, |result| {
            if let Err(e) = result {
                log::error!("{e}");
            }
            Message::NoOp
        })
    }

    pub fn handle_ipc_event(&mut self, event: Event) -> iced::Task<Message> {
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
                let window_id = window.id;

                let old_workspace_id =
                    self.windows.get(&window_id).and_then(|w| w.workspace_id);
                let new_workspace_id = window.workspace_id;

                if old_workspace_id != new_workspace_id {
                    if let Some(old_ws_id) = old_workspace_id
                        && let Some(old_ws) = self.workspaces.get_mut(&old_ws_id)
                    {
                        old_ws.windows.remove(&window_id);
                    }
                }

                self.windows.insert(window_id, window);

                if let Some(new_ws_id) = new_workspace_id
                    && let Some(new_ws) = self.workspaces.get_mut(&new_ws_id)
                {
                    let window_ref = self.windows.get(&window_id).unwrap();
                    new_ws.windows.insert(
                        window_id,
                        map_window(window_ref, &mut self.icon_cache),
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
        };
        iced::Task::none()
    }
}

fn run_event_listener(tx: tokio::sync::mpsc::UnboundedSender<Event>) {
    let mut sock = match niri_ipc::socket::Socket::connect() {
        Ok(s) => s,
        Err(e) => {
            return log::error!("Failed to connect to socket: {e}");
        }
    };

    match sock.send(Request::EventStream) {
        Ok(sent) => match sent {
            Ok(niri_ipc::Response::Handled) => {}
            Ok(other) => {
                return log::error!("Niri responded unexpectedly {other:?}");
            }
            Err(e) => {
                return log::error!("Niri handshake failed: {e}");
            }
        },
        Err(e) => {
            return log::error!("Failed to send {e}");
        }
    }

    let mut read_event = sock.read_events();

    loop {
        match read_event() {
            Ok(event) => match tx.send(event) {
                Err(e) => return log::error!("{e}"),
                Ok(_) => {}
            },
            Err(e) => {
                return log::error!("Failed to read event: {e}");
            }
        }
    }
}

async fn run_niri_request_handler(
    mut request_rx: mpsc::Receiver<Request>,
    mut socket: Socket,
) {
    while let Some(request) = request_rx.recv().await {
        if let Err(e) = socket.send(request) {
            log::error!("{e}");
        }
    }
}

pub struct NiriSubscriptionRecipe;
impl subscription::Recipe for NiriSubscriptionRecipe {
    type Output = Event;

    fn hash(&self, state: &mut subscription::Hasher) {
        std::any::TypeId::of::<Self>().hash(state);
    }

    fn stream(
        self: Box<Self>,
        _input: subscription::EventStream,
    ) -> Pin<
        Box<(dyn Stream<Item = niri_ipc::Event> + std::marker::Send + 'static)>,
    > {
        Box::pin(async_stream::stream! {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

            tokio::task::spawn_blocking(move || {
                run_event_listener(tx);
            });

            while let Some(event_result) = rx.recv().await {
                yield event_result;
            }
        })
    }
}
