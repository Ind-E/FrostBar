use iced::{
    Subscription,
    futures::{self, FutureExt, StreamExt, channel::mpsc},
};

use niri_ipc::{Action, Event, Request, WindowLayout, socket::Socket};
use std::{
    cmp::Ordering,
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tracing::error;

use crate::{
    Message,
    icon_cache::{Icon, IconCache},
    services::Service,
};

#[derive(Debug, Eq, PartialEq)]
pub enum Layout {
    Floating,
    Scrolling(usize, usize),
}

impl PartialOrd for Layout {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Layout {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Layout::Floating, Layout::Floating) => Ordering::Equal,
            (Layout::Floating, Layout::Scrolling(_, _)) => Ordering::Less,
            (Layout::Scrolling(_, _), Layout::Floating) => Ordering::Greater,
            (Layout::Scrolling(r1, c1), Layout::Scrolling(r2, c2)) => {
                r1.cmp(r2).then_with(|| c1.cmp(c2))
            }
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Window {
    pub id: u64,
    pub icon: Option<Icon>,
    pub layout: Layout,
    pub title: String,
}

impl PartialOrd for Window {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Window {
    fn cmp(&self, other: &Self) -> Ordering {
        self.layout.cmp(&other.layout)
    }
}

pub struct Workspace {
    pub output: Option<String>,
    pub idx: u8,
    pub id: u64,
    pub is_active: bool,
    pub windows: HashMap<u64, Window>,
}

fn map_window(window: &niri_ipc::Window, icon_cache: Arc<Mutex<IconCache>>) -> Window {
    let mut icon_cache = icon_cache.lock().unwrap();
    Window {
        id: window.id,
        icon: window
            .app_id
            .as_ref()
            .and_then(|app_id| icon_cache.get_icon(app_id).clone()),
        layout: window.layout.clone().into(),
        title: window.title.clone().unwrap_or("N/A".to_string()),
    }
}

impl From<WindowLayout> for Layout {
    fn from(layout: WindowLayout) -> Self {
        layout
            .pos_in_scrolling_layout
            .map_or(Layout::Floating, |l| Layout::Scrolling(l.0, l.1))
    }
}

#[derive(Debug, Clone)]
pub enum NiriEvent {
    Ready(mpsc::Sender<Request>),
    Event(Event),
    Action(Action),
}

pub struct NiriService {
    pub workspaces: HashMap<u64, Workspace>,
    pub windows: HashMap<u64, niri_ipc::Window>,
    pub hovered_workspace_id: Option<u64>,
    pub icon_cache: Arc<Mutex<IconCache>>,
    pub sender: Option<mpsc::Sender<Request>>,
}

impl Service for NiriService {
    fn subscription() -> iced::Subscription<Message> {
        Subscription::run(|| {
            async_stream::stream! {
                let (request_tx, mut request_rx) =  mpsc::channel(32);
                let (event_tx, mut event_rx) = mpsc::unbounded();
                yield NiriEvent::Ready(request_tx);

                std::thread::spawn(move || {
                    run_event_listener(event_tx);
                });

                let mut socket = Socket::connect().unwrap();
                loop{
                    futures::select! {
                        event = event_rx.next().fuse() => {
                            if let Some(event) = event {
                                yield NiriEvent::Event(event);
                            } else {
                                break;
                            }
                        },

                        request = request_rx.next().fuse() => {
                            if let Some(request) = request {
                                if let Err(e) = socket.send(request) {
                                    error!("Failed to send niri request: {e}");
                                }
                            } else {
                                break;
                            }
                        },

                        complete => break,
                    }
                }

            }
        })
        .map(Message::NiriEvent)
    }

    type Event = NiriEvent;
    fn handle_event(&mut self, event: Self::Event) -> iced::Task<Message> {
        match event {
            NiriEvent::Ready(sender) => {
                self.sender = Some(sender);
                iced::Task::none()
            }
            NiriEvent::Event(event) => self.handle_ipc_event(event),
            NiriEvent::Action(action) => {
                let Some(sender) = &self.sender else {
                    error!("Niri action triggered before sender was ready.");
                    return iced::Task::none();
                };
                let request = Request::Action(action);
                {
                    let mut sender = sender.clone();
                    iced::Task::perform(
                        async move { sender.try_send(request) },
                        |result| {
                            if let Err(e) = result {
                                error!("{e}");
                            }
                            Message::NoOp
                        },
                    )
                }
            }
        }
    }
}

impl NiriService {
    pub fn new(icon_cache: Arc<Mutex<IconCache>>) -> Self {
        Self {
            workspaces: HashMap::new(),
            windows: HashMap::new(),
            hovered_workspace_id: None,
            icon_cache,
            sender: None,
        }
    }
    fn handle_ipc_event(&mut self, event: Event) -> iced::Task<Message> {
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
                            .values()
                            .filter(|w| w.workspace_id == Some(ws.id))
                            .map(|w| (w.id, map_window(w, self.icon_cache.clone())))
                            .collect(),
                    })
                    .map(|ws| (ws.id, ws))
                    .collect();
            }
            Event::WindowsChanged { windows } => {
                self.windows = windows.into_iter().map(|w| (w.id, w)).collect();

                self.workspaces.values_mut().for_each(|ws| {
                    ws.windows = self
                        .windows
                        .values()
                        .filter(|w| w.workspace_id == Some(ws.id))
                        .map(|w| (w.id, map_window(w, self.icon_cache.clone())))
                        .collect();
                });
            }
            Event::WindowOpenedOrChanged { window } => {
                let window_id = window.id;

                let old_workspace_id =
                    self.windows.get(&window_id).and_then(|w| w.workspace_id);
                let new_workspace_id = window.workspace_id;

                if old_workspace_id != new_workspace_id
                    && let Some(old_ws_id) = old_workspace_id
                    && let Some(old_ws) = self.workspaces.get_mut(&old_ws_id)
                {
                    old_ws.windows.remove(&window_id);
                }

                self.windows.insert(window_id, window);

                if let Some(new_ws_id) = new_workspace_id
                    && let Some(new_ws) = self.workspaces.get_mut(&new_ws_id)
                {
                    let window_ref = self.windows.get(&window_id).unwrap();
                    new_ws.windows.insert(
                        window_id,
                        map_window(window_ref, self.icon_cache.clone()),
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
                for ws in self.workspaces.values_mut() {
                    if ws.output == output {
                        ws.is_active = false;
                    }
                }
                if let Some(ws) = self.workspaces.get_mut(&id) {
                    ws.is_active = true;
                }
            }
            Event::WindowLayoutsChanged { changes } => {
                for (id, layout) in changes {
                    if let Some(window) = self.windows.get_mut(&id) {
                        window.layout = layout;
                    }
                }

                self.workspaces.values_mut().for_each(|ws| {
                    ws.windows = self
                        .windows
                        .values()
                        .filter(|w| w.workspace_id == Some(ws.id))
                        .map(|w| (w.id, map_window(w, self.icon_cache.clone())))
                        .collect();
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
        iced::Task::none()
    }
}

fn run_event_listener(tx: mpsc::UnboundedSender<Event>) {
    let mut sock = match niri_ipc::socket::Socket::connect() {
        Ok(s) => s,
        Err(e) => {
            return error!("Failed to connect to socket: {e}");
        }
    };

    match sock.send(Request::EventStream) {
        Ok(sent) => match sent {
            Ok(niri_ipc::Response::Handled) => {}
            Ok(other) => {
                return error!("Niri responded unexpectedly {other:?}");
            }
            Err(e) => {
                return error!("Niri handshake failed: {e}");
            }
        },
        Err(e) => {
            return error!("Failed to send {e}");
        }
    }

    let mut read_event = sock.read_events();

    loop {
        match read_event() {
            Ok(event) => {
                if let Err(e) = tx.unbounded_send(event) {
                    return error!("{e}");
                }
            }
            Err(e) => {
                return error!("Failed to read event: {e}");
            }
        }
    }
}
