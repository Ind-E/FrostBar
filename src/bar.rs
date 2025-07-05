use chrono::{DateTime, Local};
use iced::{
    Background, Color, Element, Event, Length, Point, Rectangle, Size,
    Subscription, Task, Theme,
    advanced::{mouse, subscription},
    alignment::{Horizontal, Vertical},
    event,
    mouse::{Interaction, ScrollDelta},
    padding::{left, top},
    time::{self, Duration},
    widget::{
        Column, Container, MouseArea, Scrollable, Stack, Text, canvas, column,
        container, stack, text,
    },
    window::Id,
};
use iced_layershell::{
    actions::{IcedNewMenuSettings, MenuDirection},
    reexport::{Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings},
    to_layer_message,
};
use itertools::Itertools;
use niri_ipc::Request;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use system_tray::client::ActivateRequest;
use zbus::Connection;

use tokio::{
    process::Command as TokioCommand,
    sync::{
        Mutex as TokioMutex,
        mpsc::{self, Sender},
    },
};

use crate::{
    animated_container::AnimatedContainer,
    battery_widget::{BatteryInfo, battery_icon, fetch_battery_info},
    cava::{CavaError, CavaEvents, CavaVisualizer, write_temp_cava_config},
    config::{BAR_WIDTH, GAPS, VOLUME_PERCENT},
    dbus_proxy::PlayerProxy,
    icon_cache::{IconCache, MprisArtCache},
    mpris::{MprisEvent, MprisListener, MprisPlayer},
    niri::{IpcError, NiriEvents, NiriState, run_niri_request_handler},
    style::{no_rail, rounded_corners, tooltip_style},
    systray::{
        SysTrayInteraction, SysTrayState, SysTraySubscription, create_client,
        to_widget,
    },
    tooltip::{Tooltip, TooltipState},
    utils::align_clock,
};

#[derive(Debug, Clone)]
pub enum MouseEvent {
    Workspace(u8),
    Tooltip(container::Id),
}

#[to_layer_message(multi)]
#[derive(Debug, Clone)]
pub enum Message {
    IcedEvent(Event),
    Tick(DateTime<Local>),
    AlignClock,
    BatteryUpdate(Vec<BatteryInfo>),
    ErrorMessage(String),
    MouseEntered(MouseEvent),
    MouseExited(MouseEvent),
    MouseExitedBar,
    TooltipMeasured(container::Id, Option<Rectangle>),
    TooltipContentMeasured(container::Id, Size),
    NiriEvent(Result<niri_ipc::Event, IpcError>),
    FocusWorkspace(u8),
    FocusWindow(u64),
    MeasureWorkspaces,
    CavaUpdate(Result<String, CavaError>),
    MprisEvent(MprisEvent),
    PlayPause(String),
    NextSong(String),
    StopPlayer(String),
    ChangeVolume(i32),
    SysTrayClientCreated(Arc<system_tray::client::Client>),
    SysTrayEvent(system_tray::client::Event),
    SysTrayInteraction(SysTrayInteraction),
    CloseSysTrayMenu,
    DestroyWindow(Id),
    CreateTooltipCanvas,
    AnimationTick(std::time::Instant),
    OpenControl,
    NoOp,
}

pub struct Bar {
    pub time: DateTime<Local>,
    pub clock_aligned: bool,
    pub battery_info: (container::Id, Option<Vec<BatteryInfo>>),
    pub tooltips: HashMap<container::Id, Tooltip>,
    pub workspace_tooltips: HashMap<u8, container::Id>,
    pub niri_state: NiriState,
    pub niri_request_sender: Sender<Request>,
    pub hovered_workspace_index: Option<u8>,
    pub cava_visualizer: CavaVisualizer,
    pub icon_cache: Arc<Mutex<IconCache>>,
    pub mpris_art_cache: Arc<Mutex<MprisArtCache>>,
    pub mpris_players: HashMap<String, MprisPlayer>,
    pub systray_state: SysTrayState,
    pub systray_client: Option<Arc<system_tray::client::Client>>,
    pub systray_menu_id: Id,
    pub systray_menu_open: bool,
    pub tooltip_canvas: Id,
}

impl Bar {
    pub fn new() -> (Self, Task<Message>) {
        let battery_id = container::Id::unique();

        let battery_info = (
            battery_id.clone(),
            match fetch_battery_info() {
                Message::BatteryUpdate(info) => Some(info),
                _ => unreachable!(),
            },
        );

        let mut tooltips = HashMap::new();
        tooltips.insert(battery_id.clone(), Tooltip::default());

        let (request_tx, request_rx) = mpsc::channel(32);
        let request_socket = match niri_ipc::socket::Socket::connect() {
            Ok(sock) => Arc::new(TokioMutex::new(sock)),
            Err(e) => panic!("Failed to create niri request socket: {}", e),
        };

        tokio::spawn(run_niri_request_handler(request_rx, request_socket));

        (
            Self {
                clock_aligned: false,
                time: Local::now(),
                battery_info,
                tooltips,
                workspace_tooltips: HashMap::new(),
                niri_state: NiriState::new(IconCache::new()),
                niri_request_sender: request_tx,
                hovered_workspace_index: None,
                cava_visualizer: CavaVisualizer {
                    bars: vec![0; 10],
                    cache: iced::widget::canvas::Cache::new(),
                },
                icon_cache: Arc::new(Mutex::new(IconCache::new())),
                mpris_art_cache: Arc::new(Mutex::new(MprisArtCache::new())),
                mpris_players: HashMap::new(),
                systray_state: SysTrayState::new(),
                systray_client: None,
                systray_menu_id: Id::unique(),
                systray_menu_open: false,
                tooltip_canvas: Id::unique(),
            },
            Task::batch(vec![
                align_clock(),
                create_client(),
                Task::done(Message::CreateTooltipCanvas),
            ]),
        )
    }

    pub fn remove_id(&mut self, _id: Id) {}

    pub fn namespace(&self) -> String {
        String::from("Iced Bar")
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions: Vec<Subscription<Message>> = Vec::with_capacity(8);

        subscriptions.push(event::listen().map(Message::IcedEvent));
        subscriptions.push(
            time::every(Duration::from_secs(1)).map(|_| fetch_battery_info()),
        );

        subscriptions
            .push(subscription::from_recipe(NiriEvents).map(Message::NiriEvent));
        subscriptions.push(
            subscription::from_recipe(MprisListener).map(Message::MprisEvent),
        );
        subscriptions.push(
            subscription::from_recipe(CavaEvents {
                config_path: write_temp_cava_config()
                    .unwrap()
                    .display()
                    .to_string(),
            })
            .map(Message::CavaUpdate),
        );

        if self.clock_aligned {
            subscriptions.push(
                time::every(Duration::from_secs(60))
                    .map(|_| Message::Tick(Local::now())),
            );
        }

        if let Some(client) = &self.systray_client {
            subscriptions.push(subscription::from_recipe(SysTraySubscription {
                client: Arc::clone(client),
            }))
        }

        if self
            .tooltips
            .values()
            .any(|tip| tip.state != TooltipState::Hidden)
        {
            subscriptions.push(
                time::every(Duration::from_millis(16))
                    .map(Message::AnimationTick),
            )
        }

        Subscription::batch(subscriptions)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::CreateTooltipCanvas => Task::done(Message::NewLayerShell {
                settings: NewLayerShellSettings {
                    size: None,
                    layer: Layer::Top,
                    anchor: Anchor::Left
                        | Anchor::Right
                        | Anchor::Top
                        | Anchor::Bottom,
                    exclusive_zone: Some(-1),
                    margin: None,
                    keyboard_interactivity: KeyboardInteractivity::None,
                    use_last_output: false,
                    events_transparent: true,
                },
                id: self.tooltip_canvas,
            }),
            Message::IcedEvent(event) => {
                if let Event::Mouse(mouse::Event::CursorLeft) = event {
                    return Task::done(Message::MouseExitedBar);
                }
                // println!("{event:?}");
                Task::none()
            }
            Message::Tick(time) => {
                self.time = time;
                Task::none()
            }
            Message::OpenControl => Task::perform(
                async { TokioCommand::new("control").output().await },
                |_| Message::NoOp,
            ),
            Message::AlignClock => {
                self.clock_aligned = true;
                self.time = Local::now();
                Task::none()
            }
            Message::BatteryUpdate(info) => {
                self.battery_info.1 = Some(info);
                Task::none()
            }
            Message::NiriEvent(event) => {
                if let Ok(event) = event {
                    if let niri_ipc::Event::WorkspacesChanged { workspaces } =
                        &event
                    {
                        let old_indices: std::collections::HashSet<u8> = self
                            .niri_state
                            .workspaces
                            .values()
                            .map(|ws| ws.idx)
                            .collect();
                        let new_indices: std::collections::HashSet<u8> =
                            workspaces.iter().map(|ws| ws.idx).collect();
                        let removed_indices =
                            old_indices.difference(&new_indices);
                        for &removed_idx in removed_indices {
                            if let Some(tooltip_id) =
                                self.workspace_tooltips.remove(&removed_idx)
                            {
                                self.tooltips.remove(&tooltip_id);
                            }
                        }

                        for ws in workspaces {
                            self.workspace_tooltips
                                .entry(ws.idx)
                                .or_insert_with(container::Id::unique);
                        }

                        for (_, ws) in &self.niri_state.workspaces {
                            if let Some(tip) = self
                                .workspace_tooltips
                                .get(&ws.idx)
                                .and_then(|id| self.tooltips.get_mut(id))
                            {
                                tip.content = Some(ws.window_titles());
                            }
                        }
                    }
                    self.niri_state.on_event(event);
                }

                Task::none()
            }
            Message::FocusWorkspace(idx) => {
                let sender = self.niri_request_sender.clone();
                let request =
                    niri_ipc::Request::Action(niri_ipc::Action::FocusWorkspace {
                        reference: niri_ipc::WorkspaceReferenceArg::Index(idx),
                    });
                Task::perform(
                    async move { sender.send(request).await.ok() },
                    |_| Message::NoOp,
                )
            }
            Message::FocusWindow(id) => {
                let sender = self.niri_request_sender.clone();
                let request =
                    niri_ipc::Request::Action(niri_ipc::Action::FocusWindow {
                        id,
                    });
                Task::perform(
                    async move { sender.send(request).await.ok() },
                    |_| Message::NoOp,
                )
            }
            Message::MouseEntered(event) => {
                match event {
                    MouseEvent::Workspace(idx) => {
                        self.hovered_workspace_index = Some(idx);
                        let id = self
                            .workspace_tooltips
                            .entry(idx)
                            .or_insert_with(container::Id::unique)
                            .clone();

                        let tooltip =
                            self.tooltips.entry(id.clone()).or_default();
                        let now = std::time::Instant::now();

                        match tooltip.state {
                            TooltipState::Hidden => {
                                let workspace = self
                                    .niri_state
                                    .workspaces
                                    .values()
                                    .find(|ws| ws.idx == idx);

                                let Some(workspace) = workspace else {
                                    return Task::none();
                                };

                                let window_titles = workspace.window_titles();
                                tooltip.content = Some(window_titles);

                                tooltip.state = TooltipState::Measuring {
                                    content_measured: false,
                                    position_measured: false,
                                };

                                return container::visible_bounds(id.clone())
                                    .map(move |rect| {
                                        Message::TooltipMeasured(id.clone(), rect)
                                    });
                            }
                            TooltipState::AnimatingOut => {
                                tooltip.state = TooltipState::AnimatingIn;
                                tooltip.animating.transition(true, now);
                            }
                            _ => unreachable!(),
                        }
                    }
                    MouseEvent::Tooltip(id) => {
                        if let Some(tooltip) = self.tooltips.get_mut(&id) {
                            match tooltip.state {
                                TooltipState::Hidden => {
                                    let tooltip_content: String = if let Some(
                                        info,
                                    ) =
                                        &self.battery_info.1
                                    {
                                        info.iter()
                                            .enumerate()
                                            .map(|(i, bat)| {
                                                format!(
                                                    "Battery {}: {}% ({})",
                                                    i + 1,
                                                    (bat.percentage * 100.0)
                                                        .floor(),
                                                    bat.state
                                                )
                                            })
                                            .collect::<Vec<_>>()
                                            .join("\n")
                                    } else {
                                        "No Battery Info".to_string()
                                    };
                                    tooltip.content = Some(tooltip_content);
                                    tooltip.state = TooltipState::Measuring {
                                        content_measured: false,
                                        position_measured: false,
                                    };
                                    return container::visible_bounds(id.clone())
                                        .map(move |rect| {
                                            Message::TooltipMeasured(
                                                id.clone(),
                                                rect,
                                            )
                                        });
                                }
                                TooltipState::AnimatingOut => {
                                    let now = std::time::Instant::now();
                                    tooltip.state = TooltipState::AnimatingIn;
                                    tooltip.animating.transition(true, now);
                                }
                                _ => unreachable!(),
                            }
                        }
                    }
                };

                Task::none()
            }
            Message::TooltipMeasured(id, rect) => {
                if let Some(tooltip) = self.tooltips.get_mut(&id)
                    && let TooltipState::Measuring {
                        position_measured,
                        content_measured,
                    } = tooltip.state
                    && !position_measured
                    && let Some(rect) = rect
                {
                    let mut y = rect.y;

                    let is_workspace_tooltip = self
                        .workspace_tooltips
                        .values()
                        .any(|tooltip_id| tooltip_id == &id);

                    if is_workspace_tooltip {
                        const WORKSPACE_TOOLTIP_OFFSET: f32 = 27.5;
                        y += WORKSPACE_TOOLTIP_OFFSET;
                    } else {
                        y -= rect.height / 4.0;
                    }

                    tooltip.position = Some(Point::new(0.0, y));

                    if content_measured {
                        let now = std::time::Instant::now();
                        tooltip.state = TooltipState::AnimatingIn;
                        tooltip.animating.transition(true, now);
                    } else {
                        tooltip.state = TooltipState::Measuring {
                            content_measured,
                            position_measured: true,
                        };
                    }
                } else {
                    return container::visible_bounds(id.clone()).map(
                        move |rect| Message::TooltipMeasured(id.clone(), rect),
                    );
                }
                Task::none()
            }
            Message::TooltipContentMeasured(id, size) => {
                if let Some(tooltip) = self.tooltips.get_mut(&id)
                    && let TooltipState::Measuring {
                        content_measured,
                        position_measured,
                    } = tooltip.state
                    && !content_measured
                {
                    tooltip.size = Some(size);

                    if content_measured {
                        let now = std::time::Instant::now();
                        tooltip.state = TooltipState::AnimatingIn;
                        tooltip.animating.transition(true, now);
                    } else {
                        tooltip.state = TooltipState::Measuring {
                            content_measured: true,
                            position_measured,
                        };
                    }
                }

                Task::none()
            }
            Message::AnimationTick(now) => {
                self.tooltips
                    .values_mut()
                    .filter(|tip| {
                        tip.state == TooltipState::AnimatingOut
                            && !tip.animating.in_progress(now)
                    })
                    .for_each(|tip| {
                        tip.state = TooltipState::Hidden;
                    });
                Task::none()
            }
            Message::MouseExitedBar => {
                for tooltip in self.tooltips.values_mut().filter(|tip| {
                    tip.state != TooltipState::Hidden
                        && tip.state != TooltipState::AnimatingOut
                }) {
                    let now = std::time::Instant::now();
                    tooltip.state = TooltipState::AnimatingOut;
                    tooltip.animating.transition(false, now);
                }
                self.hovered_workspace_index = None;
                Task::none()
            }
            Message::MouseExited(event) => {
                match event {
                    MouseEvent::Workspace(idx) => {
                        if let Some(id) = self.workspace_tooltips.get(&idx) {
                            if let Some(tooltip) = self.tooltips.get_mut(id)
                                && tooltip.state != TooltipState::Hidden
                                && tooltip.state != TooltipState::AnimatingOut
                            {
                                let now = std::time::Instant::now();
                                tooltip.state = TooltipState::AnimatingOut;
                                tooltip.animating.transition(false, now);
                            }
                        }
                        self.hovered_workspace_index = None;
                    }
                    MouseEvent::Tooltip(id) => {
                        if let Some(tooltip) = self.tooltips.get_mut(&id)
                            && tooltip.state != TooltipState::Hidden
                            && tooltip.state != TooltipState::AnimatingOut
                        {
                            let now = std::time::Instant::now();
                            tooltip.state = TooltipState::AnimatingOut;
                            tooltip.animating.transition(false, now);
                        }
                    }
                };

                Task::none()
            }
            Message::ErrorMessage(msg) => {
                eprintln!("error message: {}", msg);
                Task::none()
            }
            Message::CavaUpdate(update) => {
                match update {
                    Ok(line) => {
                        self.cava_visualizer.bars = line
                            .split(";")
                            .map(|s| s.parse::<u8>().unwrap_or(0))
                            .collect();
                        self.cava_visualizer.cache.clear();
                    }
                    Err(e) => {
                        eprintln!("cava error: {}", e);
                    }
                };
                Task::none()
            }
            Message::MprisEvent(event) => {
                match event {
                    MprisEvent::PlayerAppeared {
                        name,
                        status,
                        metadata,
                    } => {
                        let mut player = MprisPlayer::new(name.clone(), status);
                        player.update_metadata(
                            &metadata,
                            &mut self.mpris_art_cache.lock().unwrap(),
                        );
                        self.mpris_players.insert(name, player);
                    }
                    MprisEvent::PlayerVanished { name } => {
                        self.mpris_players.remove(&name);
                    }
                    MprisEvent::PlaybackStatusChanged {
                        player_name,
                        status,
                    } => {
                        if let Some(player) =
                            self.mpris_players.get_mut(&player_name)
                        {
                            player.status = status;
                        }
                    }
                    MprisEvent::MetadataChanged {
                        player_name,
                        metadata,
                    } => {
                        if let Some(player) =
                            self.mpris_players.get_mut(&player_name)
                        {
                            player.update_metadata(
                                &metadata,
                                &mut self.mpris_art_cache.lock().unwrap(),
                            );
                        }
                    }
                }
                Task::none()
            }
            Message::PlayPause(player) => Task::perform(
                async {
                    if let Ok(connection) = Connection::session().await {
                        if let Ok(player) =
                            PlayerProxy::new(&connection, player).await
                        {
                            let _ = player.play_pause().await;
                        };
                    };
                },
                |_| Message::NoOp,
            ),
            Message::NextSong(player) => Task::perform(
                async {
                    if let Ok(connection) = Connection::session().await {
                        if let Ok(player) =
                            PlayerProxy::new(&connection, player).await
                        {
                            let _ = player.next().await;
                        };
                    };
                },
                |_| Message::NoOp,
            ),
            Message::ChangeVolume(delta_percent) => {
                let sign = if delta_percent >= 0 { "+" } else { "-" };
                let value = delta_percent.abs();
                Task::perform(
                    async move {
                        let _ = TokioCommand::new("wpctl")
                            .args(&[
                                "set-volume",
                                "@DEFAULT_SINK@",
                                &format!("{value}%{sign}"),
                            ])
                            .output()
                            .await;
                    },
                    |_| Message::NoOp,
                )
            }
            Message::SysTrayClientCreated(client) => {
                self.systray_state.init(client.items());
                self.systray_client = Some(client);
                Task::none()
            }
            Message::SysTrayEvent(event) => {
                self.systray_state.on_event(event);
                Task::none()
            }
            Message::MeasureWorkspaces => {
                let tooltips = self.workspace_tooltips.clone();

                Task::batch(tooltips.into_values().map(|id| {
                    container::visible_bounds(id.clone()).map(move |rect| {
                        Message::TooltipMeasured(id.clone(), rect)
                    })
                }))
            }
            Message::SysTrayInteraction(event) => {
                match event {
                    SysTrayInteraction::LeftClick(address) => {
                        if let Some(tray) = self.systray_client.clone() {
                            return Task::perform(
                                async move {
                                    match tray
                                        .activate(ActivateRequest::Default {
                                            address,
                                            x: 100,
                                            y: 100,
                                        })
                                        .await
                                    {
                                        Ok(()) => {}
                                        Err(e) => {
                                            eprintln!("sys tray error: {e}")
                                        }
                                    }
                                },
                                |_| Message::NoOp,
                            );
                        };
                    }
                    SysTrayInteraction::RightClick(address) => {
                        self.systray_state.open_menu = Some(address);
                        let id = self.systray_menu_id;
                        let task = if !self.systray_menu_open {
                            Task::done(Message::NewMenu {
                                settings: IcedNewMenuSettings {
                                    size: (400, 300),
                                    direction: MenuDirection::Up,
                                },
                                id,
                            })
                        } else {
                            iced::window::close::<()>(id).then(move |_| {
                                Task::done(Message::NewMenu {
                                    settings: IcedNewMenuSettings {
                                        size: (400, 300),
                                        direction: MenuDirection::Up,
                                    },
                                    id,
                                })
                            })
                        };
                        self.systray_menu_open = !self.systray_menu_open;

                        return task;
                    }
                }
                Task::none()
            }
            Message::CloseSysTrayMenu => {
                self.systray_menu_open = false;
                iced::window::close(self.systray_menu_id)
            }
            Message::DestroyWindow(id) => iced::window::close(id),
            Message::NoOp => Task::none(),
            _ => unreachable!(),
        }
    }

    fn view_bar(&self) -> Element<Message> {
        let time: String = self.time.format("%H\n%M").to_string();
        let cava_visualizer = MouseArea::new(
            canvas(&self.cava_visualizer)
                .width(Length::Fill)
                .height(130),
        )
        .on_scroll(|delta| {
            Message::ChangeVolume(match delta {
                ScrollDelta::Lines { x, y } => {
                    if y > 0.0 || x < 0.0 {
                        VOLUME_PERCENT
                    } else {
                        -VOLUME_PERCENT
                    }
                }
                ScrollDelta::Pixels { x, y } => {
                    if y > 0.0 || x < 0.0 {
                        VOLUME_PERCENT
                    } else {
                        -VOLUME_PERCENT
                    }
                }
            })
        });
        let mpris_art = self
            .mpris_players
            .values()
            .fold(Column::new().spacing(5).padding(5), |col, player| {
                col.push(player.to_widget())
            });

        let top_section = Container::new(
            column![
                MouseArea::new(text("󱄅").size(32))
                    .on_release(Message::OpenControl)
                    .interaction(Interaction::Pointer),
                battery_icon(
                    self.battery_info.1.as_ref(),
                    self.battery_info.0.clone()
                ),
                text(time).size(16),
                cava_visualizer,
                mpris_art,
            ]
            .align_x(Horizontal::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Top);

        let ws = self
            .niri_state
            .workspaces
            .iter()
            .sorted_by_key(|(_, ws)| ws.idx)
            .fold(Column::new(), |col, (_, ws)| {
                col.push(ws.to_widget(
                    self.hovered_workspace_index.is_some_and(|x| x == ws.idx),
                    self.workspace_tooltips.get(&ws.idx).unwrap().clone(),
                ))
            })
            .align_x(Horizontal::Center)
            .spacing(10);

        let middle_section = Container::new(
            Scrollable::new(Container::new(ws).align_y(Vertical::Center))
                .height(570)
                .on_scroll(|_| Message::MeasureWorkspaces)
                .style(no_rail),
        )
        .center_y(Length::Fill);

        let tray_items = self
            .systray_state
            .items
            .iter()
            .sorted_by_key(|(_, item)| &item.item.id)
            .map(|(address, item)| {
                to_widget(
                    address,
                    &item.item,
                    &mut self.icon_cache.lock().unwrap(),
                )
            })
            .fold(
                Column::new().padding(top(5).bottom(5)).spacing(2),
                |col, item| col.push(item),
            )
            .align_x(Horizontal::Center);

        let bottom_section = Container::new(tray_items)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Center)
            .align_y(Vertical::Bottom);

        let layout = stack![top_section, middle_section, bottom_section];

        let bar = Container::new(layout)
            .width(Length::Fixed(BAR_WIDTH as f32 - GAPS as f32 * 2.0))
            .height(Length::Fill)
            .style(rounded_corners);

        return MouseArea::new(
            Container::new(bar)
                // .style(|_| container::Style {
                //     background: Some(Background::Color(Color::from_rgba(
                //         0.7, 0.2, 0.2, 0.15,
                //     ))),
                //     ..Default::default()
                // })
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(GAPS),
        )
        .on_press(Message::CloseSysTrayMenu)
        .into();
    }

    fn view_canvas(&self) -> Element<Message> {
        Container::new(
            self.tooltips
                .iter()
                .filter(|(_id, tooltip)| tooltip.state != TooltipState::Hidden)
                .fold(Stack::new(), |stack, (id, tooltip)| {
                    let Some(content) = tooltip.content.as_ref() else {
                        return stack;
                    };

                    let is_workspace_tooltip = self
                        .workspace_tooltips
                        .values()
                        .any(|tooltip_id| tooltip_id == id);

                    let (line_height, height) = if is_workspace_tooltip {
                        (text::LineHeight::Relative(1.8), 28.8)
                    } else {
                        (text::LineHeight::Relative(1.0), 16.0)
                    };

                    let now = std::time::Instant::now();
                    let animation_progress =
                        tooltip.animating.animate_bool(0.0, 1.0, now);
                    let id_ = id.clone();

                    let width = match tooltip.state {
                        TooltipState::Measuring {
                            content_measured: false,
                            ..
                        } => Length::Shrink,
                        _ => tooltip.size.map_or(Length::Shrink, |s| {
                            Length::Fixed(s.width * animation_progress)
                        }),
                    };

                    let position =
                        tooltip.position.unwrap_or(Point::new(0.0, 0.0));

                    let (text_color, tooltip_style) = match tooltip.state {
                        TooltipState::Measuring { .. } => {
                            (Color::TRANSPARENT, tooltip_style(0.0))
                        }
                        _ => (Color::WHITE, tooltip_style(1.0)),
                    };

                    let widget = Container::new(
                        AnimatedContainer::new(
                            content
                                .lines()
                                .map(|line| {
                                    Container::new(
                                        Text::new(line)
                                            .size(16)
                                            .color(text_color)
                                            .line_height(line_height)
                                            .shaping(text::Shaping::Basic)
                                            .wrapping(text::Wrapping::None),
                                    )
                                    .width(Length::Shrink)
                                    .height(height)
                                    .clip(true)
                                })
                                .fold(Column::new(), |col, text| col.push(text)),
                            move |size| {
                                Message::TooltipContentMeasured(id_.clone(), size)
                            },
                        )
                        .style(tooltip_style)
                        .padding(7)
                        .width(width)
                        .clip(true),
                    )
                    .padding(top(position.y).left(position.x))
                    .width(Length::Shrink)
                    .height(Length::Fill);

                    stack.push(widget)
                }),
        )
        .padding(left(BAR_WIDTH as f32 - GAPS as f32 + 0.04))
        .into()
    }

    pub fn view(&self, id: Id) -> Element<Message> {
        if id == self.tooltip_canvas {
            return self.view_canvas();
        }
        if id == self.systray_menu_id {
            return Container::new(column![])
                .style(|_| container::Style {
                    background: Some(Background::Color(Color::from_rgba(
                        0.7, 0.2, 0.2, 0.15,
                    ))),
                    ..Default::default()
                })
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
        }
        self.view_bar()
    }

    pub fn style(&self, theme: &Theme) -> iced_layershell::Appearance {
        use iced_layershell::Appearance;
        Appearance {
            background_color: Color::TRANSPARENT,
            text_color: theme.palette().text,
        }
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }
}
