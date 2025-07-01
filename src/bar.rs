use chrono::{DateTime, Local};
use iced::{
    Background, Color, Element, Event, Length, Point, Rectangle, Subscription,
    Task, Theme,
    advanced::{mouse, subscription},
    alignment::{Horizontal, Vertical},
    event,
    mouse::ScrollDelta,
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
    battery_widget::{BatteryInfo, battery_icon, fetch_battery_info},
    cava::{CavaError, CavaEvents, CavaVisualizer, write_temp_cava_config},
    config::{BAR_WIDTH, GAPS, VOLUME_PERCENT},
    dbus_proxy::PlayerProxy,
    icon_cache::{IconCache, MprisArtCache},
    mpris::{MprisEvent, MprisListener, MprisPlayer},
    niri::{
        IpcError, NiriEvents, NiriState, Window, Workspace,
        run_niri_request_handler,
    },
    style::{no_rail, rounded_corners, tooltip_style},
    systray::{
        SysTrayInteraction, SysTrayState, SysTraySubscription, create_client,
        to_widget,
    },
    tooltip::{Tooltip, TooltipState},
    utils::align_clock,
};

#[derive(Debug, Clone)]
pub enum MouseEnterEvent {
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
    MouseEntered(MouseEnterEvent),
    MouseExited(MouseEnterEvent),
    MouseExitedBar,
    TooltipMeasured(container::Id, Option<Rectangle>),
    NiriEvent(Result<niri_ipc::Event, IpcError>),
    FocusWorkspace(u8),
    FocusWindow(u64),
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
    AnimationFinished(container::Id),
    DestroyWindow(Id),
    CreateTooltipCanvas,
    NoOp,
}

pub struct Bar {
    pub time: DateTime<Local>,
    pub clock_aligned: bool,
    pub battery_info: (container::Id, Option<Vec<BatteryInfo>>),
    pub tooltips: HashMap<container::Id, Tooltip>,
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
                niri_state: NiriState::default(),
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
        let mut subscriptions: Vec<Subscription<Message>> = Vec::with_capacity(5);

        subscriptions.push(event::listen().map(Message::IcedEvent));
        subscriptions.push(
            time::every(Duration::from_secs(1)).map(|_| fetch_battery_info()),
        );
        if self.clock_aligned {
            subscriptions.push(
                time::every(Duration::from_secs(60))
                    .map(|_| Message::Tick(Local::now())),
            );
        }
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
        if let Some(client) = &self.systray_client {
            subscriptions.push(subscription::from_recipe(SysTraySubscription {
                client: Arc::clone(client),
            }))
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
                // println!("{:?}", event);
                if let Ok(event) = event {
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
                    MouseEnterEvent::Workspace(idx) => {
                        self.hovered_workspace_index = Some(idx);
                    }
                    MouseEnterEvent::Tooltip(id) => {
                        if let Some(tooltip) = self.tooltips.get_mut(&id) {
                            let now = std::time::Instant::now();
                            match tooltip.state {
                                TooltipState::Hidden => {
                                    tooltip.state = TooltipState::Measuring;
                                    tooltip.animating.transition(true, now);
                                    return container::visible_bounds(id.clone())
                                        .map(move |rect| {
                                            Message::TooltipMeasured(
                                                id.clone(),
                                                rect,
                                            )
                                        });
                                }
                                TooltipState::Hiding => {
                                    tooltip.state = TooltipState::Visible;
                                    tooltip.animating.transition(true, now);
                                }
                                _ => {}
                            }
                        }
                    }
                };

                Task::none()
            }
            Message::TooltipMeasured(id, rect) => {
                if let Some(tooltip) = self.tooltips.get_mut(&id) {
                    if tooltip.state != TooltipState::Measuring {
                        return Task::none();
                    }

                    tooltip.state = TooltipState::Visible;

                    let tooltip_content: String =
                        if let Some(info) = &self.battery_info.1 {
                            info.iter()
                                .enumerate()
                                .map(|(i, bat)| {
                                    format!(
                                        "Battery {}: {}% ({})",
                                        i + 1,
                                        (bat.percentage * 100.0).floor(),
                                        bat.state
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join("\n")
                        } else {
                            "No Battery Info".to_string()
                        };
                    tooltip.content = Some(tooltip_content);

                    if let Some(rect) = rect {
                        let y = rect.y - rect.width / 4.0;
                        tooltip.position = Some(Point::new(0.0, y));
                    }
                }
                Task::none()
            }
            //TODO: make this batch tasks instead of returning early
            Message::MouseExitedBar => {
                for (id, tooltip) in self.tooltips.iter_mut() {
                    if tooltip.state == TooltipState::Visible
                        || tooltip.state == TooltipState::Measuring
                    {
                        let now = std::time::Instant::now();
                        tooltip.state = TooltipState::Hiding;
                        tooltip.animating.transition(false, now);

                        let id = id.clone();

                        let (task, handle) = Task::abortable(Task::perform(
                            async move {
                                tokio::time::sleep(
                                    std::time::Duration::from_millis(175),
                                )
                                .await;
                            },
                            move |_| Message::AnimationFinished(id.clone()),
                        ));
                        tooltip.abort_handle = Some(handle);
                        return task;
                    }
                }
                self.hovered_workspace_index = None;
                Task::none()
            }
            Message::MouseExited(event) => {
                match event {
                    MouseEnterEvent::Workspace(..) => {
                        self.hovered_workspace_index = None;
                    }
                    MouseEnterEvent::Tooltip(id) => {
                        if let Some(tooltip) = self.tooltips.get_mut(&id) {
                            if tooltip.state == TooltipState::Visible
                                || tooltip.state == TooltipState::Measuring
                            {
                                let now = std::time::Instant::now();
                                tooltip.state = TooltipState::Hiding;
                                tooltip.animating.transition(false, now);

                                let (task, handle) =
                                    Task::abortable(Task::perform(
                                        async move {
                                            tokio::time::sleep(
                                                std::time::Duration::from_millis(
                                                    175,
                                                ),
                                            )
                                            .await;
                                        },
                                        move |_| {
                                            Message::AnimationFinished(id.clone())
                                        },
                                    ));
                                tooltip.abort_handle = Some(handle);
                                return task;
                            }
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
            Message::AnimationFinished(id) => {
                if let Some(tooltip) = self.tooltips.get_mut(&id)
                    && tooltip.state == TooltipState::Hiding
                {
                    tooltip.state = TooltipState::Hidden;
                }
                Task::none()
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
                text("󱄅").size(32),
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
            .map(|(_, ws)| Workspace {
                output: &ws.output,
                idx: &ws.idx,
                is_active: &ws.is_active,
                windows: self
                    .niri_state
                    .windows
                    .iter()
                    .filter_map(|(_, w)| {
                        (w.workspace_id == Some(ws.id)).then(|| Window {
                            title: &w.title,
                            id: &w.id,
                            icon: w.app_id.as_ref().and_then(|app_id| {
                                self.icon_cache
                                    .lock()
                                    .unwrap()
                                    .get_icon(app_id)
                                    .clone()
                            }),
                        })
                    })
                    .collect::<Vec<_>>(),
            })
            .fold(Column::new(), |col, ws| {
                col.push(ws.to_widget(
                    self.hovered_workspace_index.is_some_and(|x| &x == ws.idx),
                ))
            })
            .align_x(Horizontal::Center)
            .spacing(10);

        let middle_section = Container::new(
            Scrollable::new(Container::new(ws).align_y(Vertical::Center))
                .height(570)
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
                .filter(|(_id, tooltip)| {
                    tooltip.state != TooltipState::Hidden
                        && tooltip.state != TooltipState::Measuring
                        && tooltip.content.is_some()
                        && tooltip.position.is_some()
                })
                .fold(Stack::new(), |stack, (_id, tooltip)| {
                    let now = std::time::Instant::now();
                    let content = &tooltip.content.as_ref().unwrap();
                    let width = tooltip.animating.animate_bool(0.0, 300.0, now);
                    let widget = Container::new(
                        Container::new(
                            content
                                .lines()
                                .map(|line| {
                                    Container::new(
                                        Text::new(line)
                                            .size(16)
                                            // .color(Color::from_rgba(1.0, 1.0, 1.0, progress))
                                            .line_height(
                                                text::LineHeight::Relative(1.0),
                                            )
                                            .shaping(text::Shaping::Basic)
                                            .wrapping(text::Wrapping::None),
                                    )
                                    .width(Length::Fill)
                                    .height(Length::Fixed(16.0))
                                    .clip(true)
                                })
                                .fold(Column::new(), |col, text| col.push(text)),
                        )
                        .style(tooltip_style(1.0))
                        .padding(7)
                        .width(width)
                        .clip(true),
                    )
                    .padding(
                        top(tooltip
                            .position
                            .and_then(|p| Some(p.y))
                            .unwrap_or(0.0))
                        .left(
                            tooltip
                                .position
                                .and_then(|p| Some(p.x))
                                .unwrap_or(0.0),
                        ),
                    )
                    .width(Length::Fill)
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
