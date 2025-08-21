use iced::{
    Element, Length,
    advanced::subscription,
    alignment::{Horizontal, Vertical},
    futures::{self, FutureExt, Stream, StreamExt, channel::mpsc},
    mouse::Interaction,
    padding::top,
    widget::{
        Column, Container, Image, MouseArea, Scrollable, Svg, column,
        container::{self},
        scrollable::{Direction, Scrollbar},
        text,
    },
};
use itertools::Itertools;
use niri_ipc::{
    Action, Event, Request, WindowLayout, WorkspaceReferenceArg, socket::Socket,
};
use std::cmp::Ordering;
use std::{
    collections::HashMap,
    hash::Hash,
    pin::Pin,
    sync::{Arc, Mutex},
};

use crate::{
    icon_cache::{Icon, IconCache},
    style::workspace_style,
    {Message, MouseEvent},
};

#[derive(Debug, Eq, PartialEq)]
enum Layout {
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
struct Window {
    id: u64,
    container_id: container::Id,
    icon: Option<Icon>,
    layout: Layout,
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

impl<'a> Window {
    fn to_widget(&self) -> Element<'a, Message> {
        let icon: Element<'a, Message> = match &self.icon {
            Some(Icon::Svg(handle)) => {
                Svg::new(handle.clone()).height(24).width(24).into()
            }
            Some(Icon::Raster(handle)) => {
                Image::new(handle.clone()).height(24).width(24).into()
            }
            Option::None => column![].into(),
        };

        let container = Container::new(
            MouseArea::new(icon)
                .on_right_press(Message::NiriAction(Action::FocusWindow { id: self.id }))
                .on_enter(Message::MouseEntered(MouseEvent::Window(self.id)))
                .on_exit(Message::MouseExited(MouseEvent::Window(self.id))),
        );

        container.id(self.container_id.clone()).into()
    }
}

pub struct NiriWindow {
    pub inner: niri_ipc::Window,
    pub container_id: container::Id,
}

impl NiriWindow {
    fn new(inner: niri_ipc::Window) -> Self {
        Self {
            inner,
            container_id: container::Id::unique(),
        }
    }
}

struct Workspace {
    output: Option<String>,
    idx: u8,
    id: u64,
    is_active: bool,
    windows: HashMap<u64, Window>,
}

impl<'a> Workspace {
    fn to_widget(&self, hovered: bool) -> Element<'a, Message> {
        Container::new(
            MouseArea::new(
                Container::new(
                    self.windows.values().sorted_unstable().fold(
                        Column::new()
                            .align_x(Horizontal::Center)
                            .spacing(5)
                            .push(text(self.idx - 1).size(20)),
                        |col, w| col.push(w.to_widget()),
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

fn map_window(window: &NiriWindow, icon_cache: Arc<Mutex<IconCache>>) -> Window {
    let mut icon_cache = icon_cache.lock().unwrap();
    Window {
        id: window.inner.id,
        icon: window
            .inner
            .app_id
            .as_ref()
            .and_then(|app_id| icon_cache.get_icon(app_id).clone()),
        container_id: window.container_id.clone(),
        layout: window.inner.layout.clone().into(),
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
pub enum NiriOutput {
    Ready(mpsc::Sender<Request>),
    Event(Event),
}

pub struct NiriModule {
    workspaces: HashMap<u64, Workspace>,
    pub windows: HashMap<u64, NiriWindow>,
    pub hovered_workspace_id: Option<u64>,
    icon_cache: Arc<Mutex<IconCache>>,
    sender: Option<mpsc::Sender<Request>>,
}

impl NiriModule {
    pub fn new(icon_cache: Arc<Mutex<IconCache>>) -> Self {
        Self {
            workspaces: HashMap::new(),
            windows: HashMap::new(),
            hovered_workspace_id: None,
            icon_cache,
            sender: None,
        }
    }

    pub fn to_widget<'a>(&'a self) -> Element<'a, Message> {
        let ws = self
            .workspaces
            .iter()
            .sorted_by_key(|(_, ws)| ws.idx)
            .fold(Column::new(), |col, (_, ws)| {
                col.push(
                    ws.to_widget(self.hovered_workspace_id.is_some_and(|id| id == ws.id)),
                )
            })
            .align_x(Horizontal::Center)
            .spacing(10);
        Container::new(
            Scrollable::with_direction(
                Container::new(ws).align_y(Vertical::Center),
                Direction::Vertical(Scrollbar::new().scroller_width(0).width(0)),
            )
            .height(600),
        )
        .center_y(Length::Fill)
        .into()
    }

    pub fn handle_action(&mut self, action: Action) -> iced::Task<Message> {
        let Some(sender) = &self.sender else {
            return iced::Task::none();
        };
        let request = Request::Action(action);
        {
            let mut sender = sender.clone();
            iced::Task::perform(async move { sender.try_send(request) }, |result| {
                if let Err(e) = result {
                    log::error!("{e}");
                }
                Message::NoOp
            })
        }
    }

    pub fn handle_niri_output(&mut self, output: NiriOutput) -> iced::Task<Message> {
        match output {
            NiriOutput::Ready(sender) => {
                self.sender = Some(sender);
                return iced::Task::none();
            }
            NiriOutput::Event(event) => {
                return self.handle_ipc_event(event);
            }
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
                            .filter(|w| w.inner.workspace_id == Some(ws.id))
                            .map(|w| (w.inner.id, map_window(w, self.icon_cache.clone())))
                            .collect(),
                    })
                    .map(|ws| (ws.id, ws))
                    .collect()
            }
            Event::WindowsChanged { windows } => {
                self.windows = windows
                    .into_iter()
                    .map(|w| (w.id, NiriWindow::new(w)))
                    .collect();

                self.workspaces.values_mut().for_each(|ws| {
                    ws.windows = self
                        .windows
                        .values()
                        .filter(|w| w.inner.workspace_id == Some(ws.id))
                        .map(|w| (w.inner.id, map_window(&w, self.icon_cache.clone())))
                        .collect()
                });
            }
            Event::WindowOpenedOrChanged { window } => {
                let window_id = window.id;

                let old_workspace_id = self
                    .windows
                    .get(&window_id)
                    .and_then(|w| w.inner.workspace_id);
                let new_workspace_id = window.workspace_id;

                if old_workspace_id != new_workspace_id {
                    if let Some(old_ws_id) = old_workspace_id
                        && let Some(old_ws) = self.workspaces.get_mut(&old_ws_id)
                    {
                        old_ws.windows.remove(&window_id);
                    }
                }

                self.windows.insert(window_id, NiriWindow::new(window));

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
            Event::WindowLayoutsChanged { changes } => {
                for (id, layout) in changes {
                    if let Some(window) = self.windows.get_mut(&id) {
                        window.inner.layout = layout.into()
                    }
                }

                self.workspaces.values_mut().for_each(|ws| {
                    ws.windows = self
                        .windows
                        .values()
                        .filter(|w| w.inner.workspace_id == Some(ws.id))
                        .map(|w| (w.inner.id, map_window(&w, self.icon_cache.clone())))
                        .collect()
                });
            }
        };
        iced::Task::none()
    }
}

fn run_event_listener(tx: mpsc::UnboundedSender<Event>) {
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
            Ok(event) => match tx.unbounded_send(event) {
                Err(e) => return log::error!("{e}"),
                Ok(_) => {}
            },
            Err(e) => {
                return log::error!("Failed to read event: {e}");
            }
        }
    }
}

pub struct NiriSubscriptionRecipe;
impl subscription::Recipe for NiriSubscriptionRecipe {
    type Output = NiriOutput;

    fn hash(&self, state: &mut subscription::Hasher) {
        std::any::TypeId::of::<Self>().hash(state);
    }

    fn stream(
        self: Box<Self>,
        _input: subscription::EventStream,
    ) -> Pin<Box<dyn Stream<Item = Self::Output> + Send>> {
        Box::pin(async_stream::stream! {
            let (request_tx, mut request_rx) =  mpsc::channel(32);
            let (event_tx, mut event_rx) = mpsc::unbounded();
            yield NiriOutput::Ready(request_tx);

            std::thread::spawn(move || {
                run_event_listener(event_tx);
            });

            let mut socket = Socket::connect().unwrap();
            loop{
                futures::select! {
                    event = event_rx.next().fuse() => {
                        if let Some(event) = event {
                            yield NiriOutput::Event(event);
                        } else {
                            break;
                        }
                    },

                    request = request_rx.next().fuse() => {
                        if let Some(request) = request {
                            if let Err(e) = socket.send(request) {
                                log::error!("Failed to send niri request: {e}");
                            }
                        } else {
                            break;
                        }
                    },

                    complete => break,
                }
            }

        })
    }
}
