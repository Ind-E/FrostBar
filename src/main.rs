use chrono::{DateTime, Local};
use iced::{
    Color, Element, Event, Length, Settings, Size, Subscription, Task, Theme,
    advanced::{mouse, subscription},
    alignment::{Horizontal, Vertical},
    event, theme,
    time::{self, Duration},
    widget::{Container, column, stack, text},
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
    config::{BAR_NAMESPACE, BAR_WIDTH, FIRA_CODE, FIRA_CODE_BYTES, GAPS},
    dbus_proxy::PlayerProxy,
    icon_cache::IconCache,
    modules::{
        battery::BatteryModule,
        cava::{CavaError, CavaEvents, CavaModule, write_temp_cava_config},
        mpris::{MprisEvent, MprisListener, MprisModule},
        niri::{NiriModule, NiriOutput, NiriSubscriptionRecipe},
        time::TimeModule,
    },
    style::rounded_corners,
};

mod config;
mod dbus_proxy;
mod icon_cache;
mod modules;
mod style;

pub fn main() -> iced::Result {
    if let Err(e) = tracing_log::LogTracer::init() {
        eprintln!("{}", e);
    }
    tracing_subscriber::fmt::init();
    iced::daemon(Bar::new, Bar::update, Bar::view)
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

    MouseEntered(MouseEvent),
    MouseExited(MouseEvent),
    MouseExitedBar,

    NiriOutput(NiriOutput),
    NiriAction(niri_ipc::Action),

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
    time_module: TimeModule,
    battery_module: BatteryModule,
    niri_module: NiriModule,
    mpris_module: MprisModule,
    cava_module: CavaModule,
}

impl Bar {
    pub fn new() -> (Self, Task<Message>) {
        let time_module = TimeModule::new();
        let battery_module = BatteryModule::new();

        let icon_cache = Arc::new(Mutex::new(IconCache::new()));

        let (id, open) = iced::window::open(iced::window::Settings {
            size: Size::new(BAR_WIDTH as f32, 0.0),
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
                    exclusive_zone: Some(BAR_WIDTH as i32),
                    margin: Some((GAPS, GAPS, GAPS, GAPS)),
                    input_region: Some((0, 0, BAR_WIDTH as i32, 1200)),
                    keyboard_interactivity: Some(KeyboardInteractivity::None),
                    namespace: Some(String::from(BAR_NAMESPACE)),
                    ..Default::default()
                },
                ..Default::default()
            },
            exit_on_close_request: false,
            ..Default::default()
        });

        (
            Self {
                id,
                time_module,
                battery_module,
                niri_module: NiriModule::new(icon_cache.clone()),
                mpris_module: MprisModule::new(),
                cava_module: CavaModule::new(),
            },
            Task::batch(vec![open.map(Message::OpenWindow)]),
        )
    }

    fn title(&self, _id: Id) -> String {
        String::from("feralice")
    }

    pub fn namespace(&self) -> String {
        String::from(BAR_NAMESPACE)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions: Vec<Subscription<Message>> = Vec::with_capacity(8);

        subscriptions.push(event::listen().map(Message::IcedEvent));

        subscriptions.push(
            subscription::from_recipe(NiriSubscriptionRecipe).map(Message::NiriOutput),
        );
        subscriptions
            .push(subscription::from_recipe(MprisListener).map(Message::MprisEvent));
        subscriptions.push(
            subscription::from_recipe(CavaEvents {
                config_path: write_temp_cava_config().unwrap().display().to_string(),
            })
            .map(Message::CavaUpdate),
        );

        subscriptions.push(
            time::every(Duration::from_secs(1)).map(|_| Message::Tick(Local::now())),
        );

        Subscription::batch(subscriptions)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenWindow(id) => {
                if id != self.id {
                    unreachable!();
                };
                Task::none()
            }
            Message::IcedEvent(event) => {
                if let Event::Mouse(mouse::Event::CursorLeft) = event {
                    return Task::done(Message::MouseExitedBar);
                }
                // println!("{event:?}");
                Task::none()
            }
            Message::Tick(time) => {
                self.time_module.time = time;
                self.battery_module.fetch_battery_info();
                Task::none()
            }
            Message::NiriOutput(output) => self.niri_module.handle_niri_output(output),
            Message::NiriAction(action) => self.niri_module.handle_action(action),
            Message::MouseEntered(event) => {
                match event {
                    MouseEvent::Workspace(id) => {
                        self.niri_module.hovered_workspace_id = Some(id);
                    }
                };

                Task::none()
            }
            Message::MouseExitedBar => {
                self.niri_module.hovered_workspace_id = None;
                Task::none()
            }
            Message::MouseExited(event) => {
                match event {
                    MouseEvent::Workspace(..) => {
                        self.niri_module.hovered_workspace_id = None;
                    }
                };

                Task::none()
            }
            Message::ErrorMessage(msg) => {
                error!("error message: {}", msg);
                Task::none()
            }
            Message::CavaUpdate(update) => self.cava_module.update(update),
            Message::CavaColorUpdate(gradient) => {
                self.cava_module.update_gradient(gradient);
                Task::none()
            }
            Message::MprisEvent(event) => self.mpris_module.on_event(event),
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
        let top_section = Container::new(
            column![
                text("󱄅").size(28),
                self.battery_module.to_widget(),
                self.time_module.to_widget(),
                self.cava_module.to_widget(),
                self.mpris_module.to_widget(),
            ]
            .align_x(Horizontal::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Top);

        let middle_section = self.niri_module.to_widget();

        let layout = stack![top_section, middle_section];

        let bar = Container::new(layout)
            .width(Length::Fixed(BAR_WIDTH as f32))
            .height(Length::Fill)
            // .style(bg);
            .style(rounded_corners);

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
