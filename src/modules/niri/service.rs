use std::{cmp::Ordering, io};

use iced::{
    Subscription,
    futures::{
        SinkExt as _, StreamExt as _, channel::mpsc::Sender as IcedSender,
    },
};
use niri_ipc::{Action, Event, Request, WindowLayout};
use rustc_hash::FxHashMap;
use tokio::{
    net::UnixStream,
    sync::mpsc::{self},
};
use tokio_util::codec::{Framed, LinesCodec};
use tracing::{error, info};

use crate::{
    Message,
    icon_cache::{Icon, IconCache},
    modules::{self, ModuleAction},
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
    pub title: Option<String>,
    pub app_id: Option<String>,
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
    pub windows: FxHashMap<u64, Window>,
}

#[profiling::function]
fn map_window(window: &niri_ipc::Window, icon_cache: IconCache) -> Window {
    Window {
        id: window.id,
        icon: window
            .app_id
            .as_ref()
            .and_then(|app_id| icon_cache.get_icon(app_id).clone()),
        layout: window.layout.clone().into(),
        title: window.title.clone(),
        app_id: window.app_id.clone(),
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
    Event(Result<Event, String>),
    Action(Action),
}

pub struct NiriService {
    pub workspaces: FxHashMap<u64, Workspace>,
    pub windows: FxHashMap<u64, niri_ipc::Window>,
    pub hovered_workspace_id: Option<u64>,
    pub focused_window_id: Option<u64>,
    pub icon_cache: IconCache,
    pub sender: Option<mpsc::Sender<Request>>,
}

#[profiling::all_functions]
impl NiriService {
    pub fn new(icon_cache: IconCache) -> Self {
        Self {
            workspaces: FxHashMap::default(),
            windows: FxHashMap::default(),
            hovered_workspace_id: None,
            focused_window_id: None,
            icon_cache,
            sender: None,
        }
    }

    pub fn subscription() -> Subscription<Message> {
        Subscription::run(|| {
            iced::stream::channel(100, |mut output: IcedSender<NiriEvent>| async move {
                let (request_tx, mut request_rx) = mpsc::channel(32);

                let socket_path = match std::env::var("NIRI_SOCKET") {
                    Ok(path) => path,
                    Err(e) => {
                        error!("NIRI_SOCKET environment variable not set: {e}");
                        return;
                    }
                };

                let mut ui_socket = match setup_async_socket(&socket_path).await {
                    Ok(s) => s,
                    Err(e) => {
                        error!("failed to connect to niri ui socket: {e}");
                        return;
                    }
                };

                let mut event_stream_socket = match setup_async_socket(&socket_path).await {
                    Ok(s) => s,
                    Err(e) => {
                        error!("failed to connect to niri event stream socket: {e}");
                        return;
                    }
                };

                let event_stream_request = serde_json::to_string(&Request::EventStream).unwrap();
                if let Err(e) = event_stream_socket.send(event_stream_request).await {
                    error!("failed to start niri event stream: {e}");
                    return;
                }

                match event_stream_socket.next().await {
                    Some(Ok(line)) => {
                        match serde_json::from_str::<niri_ipc::Reply>(&line) {
                            Ok(Ok(niri_ipc::Response::Handled)) => {
                                info!("niri event stream handshake successful");
                            }
                            Ok(Err(e)) => {
                                error!("niri rejected event stream request: {e}");
                                return;
                            }
                            Ok(Ok(other)) => {
                                error!("niri sent unexpected response: {other:?}");
                                return;
                            }
                            Err(e) => {
                                error!("Failed to parse niri response: {e} (Raw: {line})");
                                return;
                            }
                        }
                    }
                    _ => return,
                }

                if let Err(e) = output.try_send(NiriEvent::Ready(request_tx)) {
                    error!("niri: {e}");
                }

                loop {
                    tokio::select! {
                        maybe_line = event_stream_socket.next() => {
                            match maybe_line {
                                Some(Ok(line)) => {
                                    let event: Result<Event, _> = serde_json::from_str(&line);
                                    let send_result = output.try_send(NiriEvent::Event(
                                        event.map_err(|e| e.to_string()),
                                    ));
                                    if let Err(e) = send_result {
                                        error!("niri: {e}");
                                    }
                                }
                                Some(Err(e)) => {
                                    error!("niri event socket error: {e}");
                                    break;
                                }
                                None => {
                                    info!("niri event socket closed");
                                    break;
                                }
                            }
                        }

                        Some(request) = request_rx.recv() => {
                            match serde_json::to_string(&request) {
                                Ok(json) => {
                                    if let Err(e) = ui_socket.send(json).await {
                                        error!("failed to send request to niri: {e}");
                                    } else if let Some(Err(e)) = ui_socket.next().await {
                                        error!("niri: {e}");
                                    }
                                }
                                Err(e) => error!("failed to serialize request: {e}"),
                            }
                        }
                    }
                }

            })}) .map(|f| Message::Module(modules::ModuleMsg::Niri(f)))
    }

    pub fn update(&mut self, event: NiriEvent) -> ModuleAction {
        match event {
            NiriEvent::Ready(sender) => {
                self.sender = Some(sender);
                ModuleAction::None
            }
            NiriEvent::Event(event) => self.handle_ipc_event(event),
            NiriEvent::Action(action) => {
                let Some(sender) = &self.sender else {
                    error!("niri action triggered before sender was ready.");
                    return ModuleAction::None;
                };
                let request = Request::Action(action);
                {
                    let sender = sender.clone();
                    ModuleAction::Task(iced::Task::perform(
                        async move { sender.send(request).await },
                        |result| {
                            if let Err(e) = result {
                                error!("niri: failed to send request {e}");
                            }
                            modules::ModuleMsg::NoOp
                        },
                    ))
                }
            }
        }
    }
    fn handle_ipc_event(
        &mut self,
        event: Result<Event, String>,
    ) -> ModuleAction {
        let event = match event {
            Ok(event) => event,
            Err(e) => {
                error!("niri: {e}");
                return ModuleAction::None;
            }
        };
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
                            .map(|w| {
                                (w.id, map_window(w, self.icon_cache.clone()))
                            })
                            .collect(),
                    })
                    .map(|ws| (ws.id, ws))
                    .collect();
            }
            Event::WindowsChanged { windows } => {
                self.focused_window_id =
                    windows.iter().find_map(|w| w.is_focused.then_some(w.id));
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

                if window.is_focused {
                    self.focused_window_id = Some(window_id);
                }

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
                if Some(id) == self.focused_window_id {
                    self.focused_window_id = None;
                }
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
            Event::WindowFocusChanged { id } => {
                self.focused_window_id = id;
            }
            Event::WorkspaceUrgencyChanged { .. }
            | Event::WorkspaceActiveWindowChanged { .. }
            | Event::WindowUrgencyChanged { .. }
            | Event::KeyboardLayoutsChanged { .. }
            | Event::KeyboardLayoutSwitched { .. }
            | Event::OverviewOpenedOrClosed { .. }
            | Event::ConfigLoaded { .. }
            | Event::WindowFocusTimestampChanged { .. }
            | Event::ScreenshotCaptured { .. } => {}
        }
        ModuleAction::None
    }
}

async fn setup_async_socket(
    path: &str,
) -> io::Result<Framed<UnixStream, LinesCodec>> {
    let stream = UnixStream::connect(path).await?;
    Ok(Framed::new(stream, LinesCodec::new()))
}
