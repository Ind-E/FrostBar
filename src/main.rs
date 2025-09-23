use chrono::{DateTime, Local};
use itertools::Itertools;
use tracing::{info, warn};

use tokio::process::Command as TokioCommand;

use iced::{
    Background, Color, Element, Event, Length, Settings, Subscription, Task, Theme,
    advanced::mouse,
    alignment::{Horizontal, Vertical},
    border::rounded,
    event, theme,
    widget::{Column, Container, container, stack},
    window::Id,
};
use notify_rust::Notification;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use zbus::Connection;

use tracing::error;

use crate::{
    config::Config,
    constants::{BAR_NAMESPACE, FIRA_CODE, FIRA_CODE_BYTES},
    dbus_proxy::PlayerProxy,
    file_watcher::{FileWatcherEvent, watch_file},
    icon_cache::IconCache,
    services::{
        Service,
        battery::BatteryService,
        cava::{CavaError, CavaService},
        mpris::{MprisEvent, MprisService},
        niri::{NiriEvent, NiriService},
        time::TimeService,
    },
    utils::{CommandSpec, init_tracing, open_window, process_modules},
    views::{
        BarAlignment, battery::BatteryView, cava::CavaView, label::LabelView,
        mpris::MprisView, niri::NiriView, time::TimeView,
    },
};

mod config;
mod constants;
mod dbus_proxy;
mod file_watcher;
mod icon_cache;
mod services;
mod style;
mod utils;
mod views;

pub fn main() -> iced::Result {
    init_tracing();

    #[cfg(feature = "tracy")]
    tracy_client::Client::start();

    iced::daemon(
        || {
            let (config, config_path) = Config::init();

            Bar::new(config, config_path)
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

    FileWatcherEvent(FileWatcherEvent),

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

    OpenWindow(Id),

    Command(CommandSpec),

    ErrorMessage(String),
    NoOp,
}

pub struct Bar {
    id: Id,
    config: Config,
    config_path: PathBuf,

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

    label_views: Vec<LabelView>,
}

impl Bar {
    pub fn new(mut config: Config, config_path: PathBuf) -> (Self, Task<Message>) {
        let icon_cache = Arc::new(Mutex::new(IconCache::new()));

        let battery_service = BatteryService::new();
        let mut battery_views = vec![];

        let time_service = TimeService::new();
        let mut time_views = vec![];

        let cava_service = CavaService::new();
        let mut cava_views = vec![];

        let mpris_service = MprisService::new();
        let mut mpris_views = vec![];

        let niri_service = NiriService::new(icon_cache.clone());
        let mut niri_views = vec![];

        let mut label_views = vec![];

        process_modules(
            &mut config,
            &mut battery_views,
            &mut time_views,
            &mut cava_views,
            &mut mpris_views,
            &mut niri_views,
            &mut label_views,
        );

        let (id, open_task) = open_window(&config.layout);

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
            label_views,
            config,
            config_path,
        };

        (bar, Task::batch(vec![open_task]))
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
        subscriptions.push(watch_file(self.config_path.clone()));

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
            Message::FileWatcherEvent(event) => {
                match event {
                    FileWatcherEvent::Changed => match Config::load(&self.config_path) {
                        Ok(mut config) => {
                            process_modules(
                                &mut config,
                                &mut self.battery_views,
                                &mut self.time_views,
                                &mut self.cava_views,
                                &mut self.mpris_views,
                                &mut self.niri_views,
                                &mut self.label_views,
                            );
                            if self.config.layout == config.layout {
                                self.config = config;
                            } else {
                                self.config = config;
                                let close = iced::window::close(self.id);
                                let (id, open) = open_window(&self.config.layout);
                                self.id = id;
                                return Task::batch([close, open]);
                            }
                        }
                        Err(e) => {
                            eprintln!("{e:?}");
                            if let Err(e) = Notification::new()
                                .summary(&self.namespace())
                                .body("Failed to parse config file")
                                .show()
                            {
                                warn!(
                                    "Failed to send config parse error notification: {e:?}"
                                );
                            }
                        }
                    },
                    FileWatcherEvent::Missing => {
                        if let Err(e) = Notification::new()
                            .summary(&format!(
                                "Config file not found at {}",
                                self.config_path.display()
                            ))
                            .show()
                        {
                            warn!(
                                "Failed to send config parse error notification: {e:?}"
                            );
                        }
                    }
                }

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
                .map_or_else(iced::Task::none, |ts| ts.handle_event(time)),
            Message::UpdateBattery => self
                .battery_service
                .as_mut()
                .map_or_else(iced::Task::none, |bs| bs.handle_event(())),
            Message::NiriEvent(event) => self
                .niri_service
                .as_mut()
                .map_or_else(iced::Task::none, |ns| ns.handle_event(event)),
            Message::MouseEntered(event) => {
                match event {
                    MouseEvent::Workspace(id) => {
                        self.niri_service
                            .iter_mut()
                            .for_each(|s| s.hovered_workspace_id = Some(id));
                    }
                }

                Task::none()
            }
            Message::MouseExitedBar => {
                self.niri_service
                    .iter_mut()
                    .for_each(|s| s.hovered_workspace_id = None);
                Task::none()
            }
            Message::MouseExited(event) => {
                match event {
                    MouseEvent::Workspace(..) => {
                        self.niri_service
                            .iter_mut()
                            .for_each(|s| s.hovered_workspace_id = None);
                    }
                }

                Task::none()
            }
            Message::ErrorMessage(msg) => {
                error!("error message: {}", msg);
                Task::none()
            }
            Message::CavaUpdate(event) => self
                .cava_service
                .as_mut()
                .map_or_else(iced::Task::none, |cs| cs.handle_event(event)),
            Message::CavaColorUpdate(gradient) => self
                .cava_service
                .as_mut()
                .map_or_else(iced::Task::none, |cs| cs.update_gradient(gradient)),
            Message::MprisEvent(event) => self
                .mpris_service
                .as_mut()
                .map_or_else(iced::Task::none, |ms| ms.handle_event(event)),
            Message::PlayPause(player) => Task::perform(
                async {
                    if let Ok(connection) = Connection::session().await
                        && let Ok(player) = PlayerProxy::new(&connection, player).await
                    {
                        let _ = player.play_pause().await;
                    }
                },
                |()| Message::NoOp,
            ),
            Message::NextSong(player) => Task::perform(
                async {
                    if let Ok(connection) = Connection::session().await
                        && let Ok(player) = PlayerProxy::new(&connection, player).await
                    {
                        let _ = player.next().await;
                    }
                },
                |()| Message::NoOp,
            ),
            Message::StopPlayer(player) => Task::perform(
                async {
                    if let Ok(connection) = Connection::session().await
                        && let Ok(player) = PlayerProxy::new(&connection, player).await
                    {
                        let _ = player.stop().await;
                    }
                },
                |()| Message::NoOp,
            ),
            Message::Command(cmd) => {
                info!("Command: {cmd}");
                Task::future(async move {
                    let mut command = TokioCommand::new(cmd.command);
                    if let Some(args) = cmd.args {
                        command.args(args);
                    }

                    let _ = command.status().await;
                    Message::NoOp
                })
            }
            Message::NoOp => Task::none(),
        }
    }

    fn view_bar(&self) -> Element<'_, Message> {
        let mut top_views: Vec<(Element<Message>, usize)> = vec![];
        let mut middle_views: Vec<(Element<Message>, usize)> = vec![];
        let mut bottom_views: Vec<(Element<Message>, usize)> = vec![];

        let mut alignments = [
            (BarAlignment::Start, &mut top_views),
            (BarAlignment::Middle, &mut middle_views),
            (BarAlignment::End, &mut bottom_views),
        ];

        if let Some(service) = &self.battery_service {
            for (pos, target) in &mut alignments {
                target.extend(
                    self.battery_views
                        .iter()
                        .filter(|v| v.position.align == *pos)
                        .map(|v| (v.view(service), v.position.idx)),
                );
            }
        }

        if let Some(service) = &self.time_service {
            for (pos, target) in &mut alignments {
                target.extend(
                    self.time_views
                        .iter()
                        .filter(|v| v.position.align == *pos)
                        .map(|v| (v.view(service), v.position.idx)),
                );
            }
        }

        if let Some(service) = &self.cava_service {
            for (pos, target) in &mut alignments {
                target.extend(
                    self.cava_views
                        .iter()
                        .filter(|v| v.position.align == *pos)
                        .map(|v| (v.view(service), v.position.idx)),
                );
            }
        }

        if let Some(service) = &self.mpris_service {
            for (pos, target) in &mut alignments {
                target.extend(
                    self.mpris_views
                        .iter()
                        .filter(|v| v.position.align == *pos)
                        .map(|v| (v.view(service, &self.config.layout), v.position.idx)),
                );
            }
        }

        if let Some(service) = &self.niri_service {
            for (pos, target) in &mut alignments {
                target.extend(
                    self.niri_views
                        .iter()
                        .filter(|v| v.position.align == *pos)
                        .map(|v| (v.view(service, &self.config.style), v.position.idx)),
                );
            }
        }

        for (pos, target) in &mut alignments {
            target.extend(
                self.label_views
                    .iter()
                    .filter(|v| v.position.align == *pos)
                    .map(|v| (v.view(), v.position.idx)),
            );
        }

        let top_views: Vec<Element<Message>> = top_views
            .into_iter()
            .sorted_by_key(|(_, idx)| *idx)
            .map(|(v, _)| v)
            .collect();

        let middle_views: Vec<Element<Message>> = middle_views
            .into_iter()
            .sorted_by_key(|(_, idx)| *idx)
            .map(|(v, _)| v)
            .collect();

        let bottom_views: Vec<Element<Message>> = bottom_views
            .into_iter()
            .sorted_by_key(|(_, idx)| *idx)
            .map(|(v, _)| v)
            .collect();

        let top_section =
            Container::new(Column::with_children(top_views).align_x(Horizontal::Center))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Horizontal::Center)
                .align_y(Vertical::Top);

        let middle_section = Container::new(
            Column::with_children(middle_views).align_x(Horizontal::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center);

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
                background: Some(Background::Color(*self.config.style.background)),
                border: rounded(self.config.style.border_radius),
                ..Default::default()
            });

        bar.into()
    }

    pub fn view(&self, _id: Id) -> Element<'_, Message> {
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
