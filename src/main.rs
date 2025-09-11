use chrono::{DateTime, Local};

use directories::ProjectDirs;
use iced::{
    Background, Color, Element, Event, Length, Settings, Size, Subscription, Task, Theme,
    advanced::mouse,
    alignment::{Horizontal, Vertical},
    border::rounded,
    event, theme,
    widget::{Column, Container, Text, container, stack},
    window::{
        Id,
        settings::{
            Anchor, KeyboardInteractivity, Layer, LayerShellSettings, PlatformSpecific,
        },
    },
};
use std::{
    process::Command,
    sync::{Arc, Mutex},
    thread,
};
use zbus::Connection;

use tracing::error;

use crate::{
    config::Config,
    constants::{BAR_NAMESPACE, FIRA_CODE, FIRA_CODE_BYTES},
    dbus_proxy::PlayerProxy,
    icon_cache::IconCache,
    services::{
        Service,
        battery::BatteryService,
        cava::{CavaError, CavaService},
        mpris::{MprisEvent, MprisService},
        niri::{NiriEvent, NiriService},
        time::TimeService,
    },
    views::{
        battery::BatteryView, cava::CavaView, mpris::MprisView, niri::NiriView,
        time::TimeView,
    },
};

mod config;
mod constants;
mod dbus_proxy;
mod icon_cache;
mod services;
mod style;
mod views;

pub fn main() -> iced::Result {
    tracing_subscriber::fmt::init();

    #[cfg(feature = "tracy")]
    tracy_client::Client::start();

    iced::daemon(
        || {
            let config = {
                let Some(project_dir) = ProjectDirs::from("", "", BAR_NAMESPACE) else {
                    std::process::exit(1);
                };
                let config_path =
                    project_dir.config_dir().to_path_buf().join("config.kdl");
                match Config::load_or_create(&config_path) {
                    Err(e) => {
                        eprintln!("{e}");
                        std::process::exit(1)
                    }
                    Ok(config) => config,
                }
            };

            Bar::new(config)
        },
        Bar::update,
        Bar::view,
    )
    .subscription(Bar::subscription)
    .style(Bar::style)
    .title(Bar::title)
    .theme(Bar::theme)
    .settings(Settings {
        fonts: vec![FIRA_CODE_BYTES.into()],
        default_font: FIRA_CODE,
        antialiasing: false,
        ..Default::default()
    })
    .run()
}

#[derive(Debug, Clone)]
pub enum MouseEvent {
    Workspace(u64),
}

#[derive(Debug, Clone)]
pub enum Message {
    IcedEvent(Event),

    Tick(DateTime<Local>),
    UpdateBattery,

    MouseEntered(MouseEvent),
    MouseExited(MouseEvent),
    MouseExitedBar,

    NiriEvent(NiriEvent),

    CavaUpdate(Result<String, CavaError>),
    CavaColorUpdate(Option<Vec<Color>>),

    MprisEvent(MprisEvent),
    PlayPause(String),
    NextSong(String),
    StopPlayer(String),
    ChangeVolume(i32),

    OpenWindow(Id),

    Command(String),

    ErrorMessage(String),
    NoOp,
}

pub struct Bar {
    id: Id,
    config: Config,

    time_views: Vec<TimeView>,
    time_service: Option<TimeService>,

    battery_views: Vec<BatteryView>,
    battery_service: Option<BatteryService>,

    niri_views: Vec<NiriView>,
    niri_service: Option<NiriService>,

    mpris_views: Vec<MprisView>,
    mpris_service: Option<MprisService>,

    cava_views: Vec<CavaView>,
    cava_service: Option<CavaService>,
}

impl Bar {
    pub fn new(config: Config) -> (Self, Task<Message>) {
        let icon_cache = Arc::new(Mutex::new(IconCache::new()));

        let time_service = TimeService::new();
        let time_views = vec![TimeView::new()];

        let battery_service = BatteryService::new();
        let battery_views = vec![BatteryView::new()];

        let niri_service = NiriService::new(icon_cache.clone());
        let niri_views = vec![NiriView::new()];

        let mpris_service = MprisService::new(config.modules.cava.clone());
        let mpris_views = vec![MprisView::new()];

        let cava_service = CavaService::new();
        let cava_views = vec![CavaView::new()];

        let (id, open) = iced::window::open(iced::window::Settings {
            size: Size::new(config.layout.width as f32, 0.0),
            decorations: false,
            resizable: false,
            minimizable: false,
            transparent: true,
            platform_specific: PlatformSpecific {
                layer_shell: LayerShellSettings {
                    layer: Some(Layer::Top),
                    anchor: Some(
                        Anchor::LEFT | Anchor::TOP | Anchor::BOTTOM | Anchor::RIGHT,
                    ),
                    exclusive_zone: Some(config.layout.width as i32),
                    margin: Some((
                        config.layout.gaps,
                        config.layout.gaps,
                        config.layout.gaps,
                        config.layout.gaps,
                    )),
                    input_region: Some((0, 0, config.layout.width as i32, 1200)),
                    keyboard_interactivity: Some(KeyboardInteractivity::None),
                    namespace: Some(String::from(BAR_NAMESPACE)),
                    ..Default::default()
                },
                ..Default::default()
            },
            exit_on_close_request: false,
            ..Default::default()
        });

        let bar = Self {
            id,
            time_service: Some(time_service),
            time_views,
            battery_service: Some(battery_service),
            battery_views,
            niri_service: Some(niri_service),
            niri_views,
            mpris_service: Some(mpris_service),
            mpris_views,
            cava_service: Some(cava_service),
            cava_views,
            config,
        };

        (bar, Task::batch(vec![open.map(Message::OpenWindow)]))
    }

    fn title(&self, _id: Id) -> String {
        String::from("FrostBar")
    }

    pub fn namespace(&self) -> String {
        String::from(BAR_NAMESPACE)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions: Vec<Subscription<Message>> = Vec::with_capacity(8);

        subscriptions.push(event::listen().map(Message::IcedEvent));

        if self.time_service.is_some() {
            subscriptions.push(TimeService::subscription());
        }

        if self.battery_service.is_some() {
            subscriptions.push(BatteryService::subscription());
        }

        if self.niri_service.is_some() {
            subscriptions.push(NiriService::subscription());
        }

        if self.mpris_service.is_some() {
            subscriptions.push(MprisService::subscription());
        }

        if self.cava_service.is_some() {
            subscriptions.push(CavaService::subscription());
        }

        Subscription::batch(subscriptions)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenWindow(id) => {
                debug_assert!(id == self.id);
                Task::none()
            }
            Message::IcedEvent(event) => {
                if let Event::Mouse(mouse::Event::CursorLeft) = event {
                    return Task::done(Message::MouseExitedBar);
                }
                // println!("{event:?}");
                Task::none()
            }
            Message::Tick(time) => self
                .time_service
                .as_mut()
                .map(|ts| ts.handle_event(time))
                .unwrap_or_else(iced::Task::none),
            Message::UpdateBattery => self
                .battery_service
                .as_mut()
                .map(|bs| bs.handle_event(()))
                .unwrap_or_else(iced::Task::none),
            Message::NiriEvent(event) => self
                .niri_service
                .as_mut()
                .map(|ns| ns.handle_event(event))
                .unwrap_or_else(iced::Task::none),
            Message::MouseEntered(event) => {
                match event {
                    MouseEvent::Workspace(id) => {
                        self.niri_service
                            .as_mut()
                            .map(|s| s.hovered_workspace_id = Some(id));
                    }
                };

                Task::none()
            }
            Message::MouseExitedBar => {
                self.niri_service
                    .as_mut()
                    .map(|s| s.hovered_workspace_id = None);
                Task::none()
            }
            Message::MouseExited(event) => {
                match event {
                    MouseEvent::Workspace(..) => {
                        self.niri_service
                            .as_mut()
                            .map(|s| s.hovered_workspace_id = None);
                    }
                };

                Task::none()
            }
            Message::ErrorMessage(msg) => {
                error!("error message: {}", msg);
                Task::none()
            }
            Message::CavaUpdate(event) => self
                .cava_service
                .as_mut()
                .map(|cs| cs.handle_event(event))
                .unwrap_or_else(iced::Task::none),
            Message::CavaColorUpdate(gradient) => self
                .cava_service
                .as_mut()
                .map(|cs| cs.update_gradient(gradient))
                .unwrap_or_else(iced::Task::none),
            Message::MprisEvent(event) => self
                .mpris_service
                .as_mut()
                .map(|ms| ms.handle_event(event))
                .unwrap_or_else(iced::Task::none),
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
            Message::StopPlayer(player) => Task::perform(
                async {
                    if let Ok(connection) = Connection::session().await {
                        if let Ok(player) = PlayerProxy::new(&connection, player).await {
                            let _ = player.stop().await;
                        };
                    };
                },
                |_| Message::NoOp,
            ),
            Message::Command(cmd) => {
                thread::spawn(|| {
                    if let Err(e) = Command::new(cmd).status() {
                        error!("{e}");
                    }
                });
                Task::none()
            }
            Message::ChangeVolume(delta_percent) => {
                let sign = if delta_percent >= 0 { "+" } else { "-" };
                let value = delta_percent.abs();
                thread::spawn(move || {
                    if let Err(e) = Command::new("wpctl")
                        .args([
                            "set-volume",
                            "@DEFAULT_SINK@",
                            &format!("{value}%{sign}"),
                        ])
                        .output()
                    {
                        error!("{e}");
                    }
                });

                Task::none()
            }
            Message::NoOp => Task::none(),
        }
    }

    fn view_bar(&self) -> Element<Message> {
        let mut top_views: Vec<Element<Message>> = vec![];

        top_views.push(Text::new("󱄅").size(28).into());

        if let Some(battery_service) = &self.battery_service {
            top_views.extend(
                self.battery_views
                    .iter()
                    .map(|v| v.view(battery_service, &self.config)),
            )
        }

        if let Some(time_service) = &self.time_service {
            top_views.extend(
                self.time_views
                    .iter()
                    .map(|v| v.view(time_service, &self.config)),
            )
        }

        if let Some(cava_service) = &self.cava_service {
            top_views.extend(
                self.cava_views
                    .iter()
                    .map(|v| v.view(cava_service, &self.config)),
            )
        }

        if let Some(mpris_service) = &self.mpris_service {
            top_views.extend(
                self.mpris_views
                    .iter()
                    .map(|v| v.view(mpris_service, &self.config)),
            )
        }

        let top_section =
            Container::new(Column::with_children(top_views).align_x(Horizontal::Center))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Horizontal::Center)
                .align_y(Vertical::Top);

        let mut middle_views: Vec<Element<Message>> = vec![];

        if let Some(niri_service) = &self.niri_service {
            middle_views.extend(
                self.niri_views
                    .iter()
                    .map(|v| v.view(niri_service, &self.config)),
            )
        }

        let middle_section = Container::new(
            Column::with_children(middle_views).align_x(Horizontal::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center);

        let mut bottom_views: Vec<Element<Message>> = vec![];

        let bottom_section = Container::new(
            Column::with_children(bottom_views).align_x(Horizontal::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Bottom);

        let layout = stack![top_section, middle_section, bottom_section];

        let bar = Container::new(layout)
            .width(Length::Fixed(self.config.layout.width as f32))
            .height(Length::Fill)
            .style(|_theme| container::Style {
                background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.8))),
                border: rounded(self.config.layout.border_radius),
                ..Default::default()
            });

        bar.into()
    }

    pub fn view(&self, _id: Id) -> Element<Message> {
        self.view_bar()
    }

    pub fn style(&self, theme: &Theme) -> theme::Style {
        theme::Style {
            background_color: Color::TRANSPARENT,
            text_color: theme.palette().text,
        }
    }

    pub fn theme(&self, _id: Id) -> Theme {
        Theme::Dark
    }
}
