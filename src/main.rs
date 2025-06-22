use chrono::{DateTime, Local};
use iced::{
    Color, Element, Event, Font, Length, Rectangle, Subscription, Task, Theme,
    advanced::{mouse, subscription},
    alignment::{Horizontal, Vertical},
    event,
    font::{Family, Weight},
    mouse::ScrollDelta,
    padding::top,
    time::{self, Duration},
    widget::{
        Column, Container, MouseArea, Scrollable, Text, canvas, column, container, stack, text,
    },
    window::Id,
};
use iced_layershell::{
    actions::IcedNewPopupSettings,
    build_pattern::{MainSettings, daemon},
    reexport::{Anchor, KeyboardInteractivity},
    settings::{LayerShellSettings, StartMode},
    to_layer_message,
};
use itertools::Itertools;
use niri_ipc::Request;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use system_tray::client::ActivateRequest;
use zbus::Connection;

use tokio::{
    process::Command as TokioCommand,
    sync::{
        Mutex as TokioMutex,
        mpsc::{self, Sender},
    },
    time::Instant,
};

use crate::{
    battery_widget::{BatteryInfo, battery_icon, fetch_battery_info},
    cava::{CavaError, CavaEvents, CavaVisualizer, write_temp_cava_config},
    dbus_proxy::PlayerProxy,
    icon_cache::{IconCache, MprisArtCache},
    mpris::{MprisEvent, MprisListener, MprisPlayer},
    niri::{IpcError, NiriEvents, NiriState, Window, Workspace, run_niri_request_handler},
    style::{no_rail, rounded_corners, tooltip_style},
    systray::{SysTrayInteraction, SysTrayState, SysTraySubscription, create_client, to_widget},
    tooltip::{Hidden, Tooltip, TooltipState},
    utils::align_clock,
};

extern crate starship_battery as battery;

mod battery_widget;
mod cava;
mod dbus_proxy;
mod icon_cache;
mod mpris;
mod niri;
mod style;
mod systray;
mod tooltip;
mod utils;

const BAR_WIDTH: u32 = 45;
const GAPS: i32 = 3;
const ANIMATION_DURATION: Duration = Duration::from_millis(175);
const VOLUME_PERCENT: i32 = 3;

const FIRA_CODE_BYTES: &[u8] = include_bytes!("../assets/FiraCodeNerdFontMono-Medium.ttf");
const FIRA_CODE: Font = Font {
    family: Family::Name("FiraCode Nerd Font Mono"),
    weight: Weight::Medium,
    ..Font::DEFAULT
};

#[tokio::main]
pub async fn main() -> Result<(), iced_layershell::Error> {
    daemon(Bar::namespace, Bar::update, Bar::view, Bar::remove_id)
        .subscription(Bar::subscription)
        .style(Bar::style)
        .theme(Bar::theme)
        .settings(MainSettings {
            fonts: vec![FIRA_CODE_BYTES.into()],
            default_font: FIRA_CODE,
            layer_settings: LayerShellSettings {
                size: Some((BAR_WIDTH, 0)),
                exclusive_zone: BAR_WIDTH as i32 - GAPS,
                anchor: Anchor::Left | Anchor::Top | Anchor::Bottom,
                margin: (GAPS, 0, GAPS, GAPS),
                keyboard_interactivity: KeyboardInteractivity::None,
                start_mode: StartMode::Active,
                ..Default::default()
            },
            ..Default::default()
        })
        .run_with(|| Bar::new())
}

struct Bar {
    time: DateTime<Local>,
    clock_aligned: bool,
    battery_info: (container::Id, Option<Vec<BatteryInfo>>),
    tooltips: HashMap<container::Id, TooltipState>,
    niri_state: NiriState,
    niri_request_sender: Sender<Request>,
    hovered_workspace_index: Option<u8>,
    cava_visualizer: CavaVisualizer,
    icon_cache: Arc<Mutex<IconCache>>,
    mpris_art_cache: Arc<Mutex<MprisArtCache>>,
    mpris_players: HashMap<String, MprisPlayer>,
    systray_state: SysTrayState,
    systray_client: Option<Arc<system_tray::client::Client>>,
}

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
    AnimationTick(Instant),
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
    NoOp,
}

impl Bar {
    fn new() -> (Self, Task<Message>) {
        let battery_id = container::Id::unique();
        let battery_window_id = Id::unique();

        let battery_info = (
            battery_id.clone(),
            match fetch_battery_info() {
                Message::BatteryUpdate(info) => Some(info),
                _ => unreachable!(),
            },
        );

        let mut tooltips = HashMap::new();
        tooltips.insert(
            battery_id.clone(),
            TooltipState::Hidden(Tooltip {
                id: battery_window_id.clone(),
                state: Hidden,
            }),
        );

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
            },
            Task::batch(vec![align_clock(), create_client()]),
        )
    }

    fn remove_id(&mut self, _id: Id) {}

    fn namespace(&self) -> String {
        String::from("Iced Bar")
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions: Vec<Subscription<Message>> = Vec::with_capacity(5);

        subscriptions.push(event::listen().map(Message::IcedEvent));
        subscriptions.push(time::every(Duration::from_secs(1)).map(|_| fetch_battery_info()));
        if self.clock_aligned {
            subscriptions
                .push(time::every(Duration::from_secs(60)).map(|_| Message::Tick(Local::now())));
        }
        for tooltip in self.tooltips.values() {
            if matches!(
                tooltip,
                TooltipState::AnimatingIn { .. } | TooltipState::AnimatingOut { .. }
            ) {
                subscriptions.push(
                    time::every(Duration::from_millis(1000 / 60))
                        .map(|_| Message::AnimationTick(Instant::now())),
                );
                break;
            }
        }
        subscriptions.push(subscription::from_recipe(NiriEvents).map(Message::NiriEvent));
        subscriptions.push(subscription::from_recipe(MprisListener).map(Message::MprisEvent));
        subscriptions.push(
            subscription::from_recipe(CavaEvents {
                config_path: write_temp_cava_config().unwrap().display().to_string(),
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

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
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
                let request = niri_ipc::Request::Action(niri_ipc::Action::FocusWorkspace {
                    reference: niri_ipc::WorkspaceReferenceArg::Index(idx),
                });
                Task::perform(async move { sender.send(request).await.ok() }, |_| {
                    Message::NoOp
                })
            }
            Message::FocusWindow(id) => {
                let sender = self.niri_request_sender.clone();
                let request = niri_ipc::Request::Action(niri_ipc::Action::FocusWindow { id });
                Task::perform(async move { sender.send(request).await.ok() }, |_| {
                    Message::NoOp
                })
            }
            Message::MouseEntered(event) => {
                match event {
                    MouseEnterEvent::Workspace(idx) => {
                        self.hovered_workspace_index = Some(idx);
                    }
                    MouseEnterEvent::Tooltip(id) => {
                        let tooltip_content: String = if let Some(info) = &self.battery_info.1 {
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

                        match &self.tooltips.get(&id).unwrap() {
                            TooltipState::Hidden(tooltip) => {
                                self.tooltips.insert(
                                    id.clone(),
                                    TooltipState::AnimatingIn(
                                        tooltip.clone().animate_in(tooltip_content),
                                    ),
                                );
                                return container::visible_bounds(id.clone())
                                    .map(move |rect| Message::TooltipMeasured(id.clone(), rect));
                            }
                            TooltipState::AnimatingOut(tooltip) => {
                                self.tooltips.insert(
                                    id,
                                    TooltipState::AnimatingIn(tooltip.clone().animate_in()),
                                );
                            }
                            _ => {}
                        };
                    }
                };

                Task::none()
            }
            Message::MouseExitedBar => {
                self.tooltips.values_mut().for_each(|t| match t {
                    TooltipState::Visible(tooltip) => {
                        *t = TooltipState::AnimatingOut(tooltip.clone().animate_out());
                    }
                    TooltipState::AnimatingIn(tooltip) => {
                        *t = TooltipState::AnimatingOut(tooltip.clone().animate_out());
                    }
                    _ => {}
                });
                self.hovered_workspace_index = None;
                Task::none()
            }
            Message::MouseExited(event) => {
                match event {
                    MouseEnterEvent::Workspace(..) => {
                        self.hovered_workspace_index = None;
                    }
                    MouseEnterEvent::Tooltip(id) => {
                        match &self.tooltips.get(&id).unwrap() {
                            TooltipState::Visible(tooltip) => {
                                self.tooltips.insert(
                                    id,
                                    TooltipState::AnimatingOut(tooltip.clone().animate_out()),
                                );
                            }
                            TooltipState::AnimatingIn(tooltip) => {
                                self.tooltips.insert(
                                    id,
                                    TooltipState::AnimatingOut(tooltip.clone().animate_out()),
                                );
                            }
                            _ => {}
                        };
                    }
                };

                Task::none()
            }
            Message::AnimationTick(now) => {
                for t in self.tooltips.values_mut() {
                    match t {
                        TooltipState::AnimatingIn(tooltip) => {
                            if now.duration_since(tooltip.state.start) >= ANIMATION_DURATION {
                                *t = TooltipState::Visible(tooltip.clone().to_visible());
                            }
                        }
                        TooltipState::AnimatingOut(tooltip) => {
                            if now.duration_since(tooltip.state.start) >= ANIMATION_DURATION {
                                let task = iced::window::close(tooltip.clone().id);
                                *t = TooltipState::Hidden(tooltip.clone().to_hidden());
                                return task;
                            }
                        }
                        TooltipState::Hidden(_tooltip) => {}
                        TooltipState::Visible(_tooltip) => {}
                    }
                }
                Task::none()
            }
            Message::TooltipMeasured(id, rect) => {
                if let Some(rect) = rect {
                    Task::done(Message::NewPopUp {
                        settings: IcedNewPopupSettings {
                            size: (400, 300),
                            position: (
                                BAR_WIDTH as i32 - GAPS as i32 - 1,
                                (rect.y - rect.width / 4.0) as i32,
                            ),
                        },
                        id: self.tooltips.get(&id).unwrap().id(),
                    })
                } else {
                    Task::none()
                }
            }
            Message::ErrorMessage(msg) => {
                eprintln!("{}", msg);
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
                        eprintln!("{}", e);
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
                        player
                            .update_metadata(&metadata, &mut self.mpris_art_cache.lock().unwrap());
                        self.mpris_players.insert(name, player);
                    }
                    MprisEvent::PlayerVanished { name } => {
                        self.mpris_players.remove(&name);
                    }
                    MprisEvent::PlaybackStatusChanged {
                        player_name,
                        status,
                    } => {
                        if let Some(player) = self.mpris_players.get_mut(&player_name) {
                            player.status = status;
                        }
                    }
                    MprisEvent::MetadataChanged {
                        player_name,
                        metadata,
                    } => {
                        if let Some(player) = self.mpris_players.get_mut(&player_name) {
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
                        if let Ok(player) = PlayerProxy::new(&connection, player).await {
                            let _ = player.play_pause().await;
                        };
                    };
                },
                |_| Message::NoOp,
            ),
            Message::NextSong(player) => Task::perform(
                async {
                    if let Ok(connection) = Connection::session().await {
                        if let Ok(player) = PlayerProxy::new(&connection, player).await {
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
                            .args(&["set-volume", "@DEFAULT_SINK@", &format!("{value}%{sign}")])
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
                self.systray_state
                    .on_event(event, &mut self.icon_cache.lock().unwrap());
                Task::none()
            }
            Message::SysTrayInteraction(event) => {
                match event {
                    SysTrayInteraction::LeftClick(id) => {
                        if let Some(tray) = self.systray_client.clone() {
                            return Task::perform(
                                async move {
                                    match tray
                                        .activate(ActivateRequest::Default {
                                            address: id,
                                            x: 0,
                                            y: 0,
                                        })
                                        .await
                                    {
                                        Ok(_) => {}
                                        Err(e) => eprintln!("{e}"),
                                    }
                                },
                                |_| Message::NoOp,
                            );
                        };
                    }
                    SysTrayInteraction::RightClick(id) => {
                        // self.systray_state.items.get(&id).and_then(|item| {
                        // if let Some(tray) = &self.systray_client {
                        // tray.menu()
                        // }
                        // None
                        // });
                    }
                }
                Task::none()
            }
            Message::NoOp => Task::none(),
            _ => unreachable!(),
        }
    }

    fn view(&self, id: Id) -> Element<Message> {
        for (_, tooltip) in self
            .tooltips
            .iter()
            .filter(|(_, tooltip)| tooltip.id() == id)
        {
            let progress = tooltip.progress();
            return Container::new(
                tooltip
                    .content()
                    .lines()
                    .map(|line| {
                        Container::new(
                            Text::new(line)
                                .size(16)
                                // .color(Color::from_rgba(1.0, 1.0, 1.0, progress))
                                .line_height(text::LineHeight::Relative(1.0))
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
            .width(progress * 300.0)
            .clip(true)
            .into();
        }
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
                battery_icon(self.battery_info.1.as_ref(), self.battery_info.0.clone()),
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
                                self.icon_cache.lock().unwrap().get_icon(app_id).clone()
                            }),
                        })
                    })
                    .collect::<Vec<_>>(),
            })
            .fold(Column::new(), |col, ws| {
                col.push(ws.to_widget(self.hovered_workspace_index.is_some_and(|x| &x == ws.idx)))
            })
            .align_x(Horizontal::Center)
            .spacing(12);

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
            .sorted_by_key(|(_, item)| &item.id)
            .map(|(id, item)| to_widget(id, item, &mut self.icon_cache.lock().unwrap()))
            .fold(Column::new(), |col, item| col.push(item))
            .align_x(Horizontal::Center);

        let bottom_section = Container::new(tray_items)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Center)
            .align_y(Vertical::Bottom);

        let layout = stack![top_section, middle_section, bottom_section];

        Container::new(layout)
            .width(Length::Fixed(BAR_WIDTH as f32 - GAPS as f32))
            .height(Length::Fill)
            .padding(top(GAPS as f32).bottom(GAPS as f32))
            .style(rounded_corners)
            .into()
    }

    fn style(&self, theme: &Theme) -> iced_layershell::Appearance {
        use iced_layershell::Appearance;
        Appearance {
            background_color: Color::TRANSPARENT,
            // Color::from_rgba(0.0, 0.0, 0.0, 0.8),
            text_color: theme.palette().text,
        }
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }
}
