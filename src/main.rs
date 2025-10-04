use chrono::{DateTime, Local};
use iced_layershell::settings::{LayerShellSettings, StartMode};
use itertools::Itertools;
use tracing::{debug, info, warn};

use tokio::process::Command as TokioCommand;

use iced::{
    Alignment, Background, Color, Element, Event, Length, Pixels, Subscription,
    Task, Theme,
    advanced::mouse,
    border::rounded,
    event,
    padding::left,
    theme,
    widget::{Column, Container, Row, container, stack},
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
    config::{Config, MediaControl},
    constants::{BAR_NAMESPACE, FIRA_CODE, FIRA_CODE_BYTES},
    dbus_proxy::PlayerProxy,
    file_watcher::{FileWatcherEvent, watch_file},
    icon_cache::IconCache,
    services::{
        Service,
        battery::BatteryService,
        cava::CavaService,
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
mod popup_tooltip;
mod services;
mod style;
mod utils;
mod views;

#[cfg(feature = "tracy-allocations")]
#[global_allocator]
static GLOBAL: tracy_client::ProfiledAllocator<std::alloc::System> =
    tracy_client::ProfiledAllocator::new(std::alloc::System, 100);

pub fn main() -> iced_layershell::Result {
    #[cfg(feature = "tracy")]
    tracy_client::Client::start();

    iced_layershell::daemon(
        || {
            let (config, config_path, config_dir) = Config::init();

            let log_dir = init_tracing(&config_dir);

            info!("starting version {}", env!("CARGO_PKG_VERSION"));
            info!("saving logs to {:?}", log_dir);

            Bar::new(config, config_path)
        },
        Bar::namespace,
        Bar::update,
        Bar::view,
    )
    .subscription(Bar::subscription)
    .style(Bar::style)
    .title(Bar::title)
    .theme(Bar::theme)
    .settings(iced_layershell::Settings {
        id: Some(BAR_NAMESPACE.to_string()),
        layer_settings: LayerShellSettings {
            start_mode: StartMode::Background,
            ..Default::default()
        },
        fonts: vec![FIRA_CODE_BYTES.into()],
        default_font: FIRA_CODE,
        default_text_size: Pixels(16.0),
        antialiasing: true,
        virtual_keyboard_support: None,
        with_connection: None,
    })
    .run()
}

#[derive(Debug, Clone)]
pub enum MouseEvent {
    Workspace(u64),
}

#[iced_layershell::to_layer_message(multi)]
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

    CavaUpdate(Option<String>),
    CavaColorUpdate(Option<Vec<Color>>),

    MprisEvent(MprisEvent),
    MediaControl(MediaControl, String),

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

#[profiling::all_functions]
impl Bar {
    pub fn new(
        mut config: Config,
        config_path: PathBuf,
    ) -> (Self, Task<Message>) {
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

        (bar, open_task)
    }

    fn title(&self, _id: Id) -> Option<String> {
        Some(String::from(BAR_NAMESPACE))
    }

    pub fn namespace() -> String {
        String::from(BAR_NAMESPACE)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions: Vec<Subscription<Message>> =
            Vec::with_capacity(8);

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
            Message::IcedEvent(event) => {
                if let Event::Mouse(mouse::Event::CursorLeft) = event {
                    return Task::done(Message::MouseExitedBar);
                }

                Task::none()
            }
            Message::FileWatcherEvent(event) => {
                match event {
                    FileWatcherEvent::Changed => {
                        match Config::load(&self.config_path) {
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
                                    return Task::none();
                                }

                                let old_layout = &self.config.layout;
                                let new_layout = &config.layout;
                                let mut tasks = Vec::new();

                                if new_layout.anchor != old_layout.anchor {
                                    tasks.push(Task::done(
                                        Message::AnchorSizeChange {
                                            id: self.id,
                                            anchor: new_layout.anchor.into(),
                                            size: if new_layout
                                                .anchor
                                                .vertical()
                                            {
                                                (new_layout.width, 0)
                                            } else {
                                                (0, new_layout.width)
                                            },
                                        },
                                    ));
                                } else if new_layout.width != old_layout.width {
                                    tasks.push(Task::done(
                                        Message::SizeChange {
                                            id: self.id,
                                            size: if new_layout
                                                .anchor
                                                .vertical()
                                            {
                                                (new_layout.width, 0)
                                            } else {
                                                (0, new_layout.width)
                                            },
                                        },
                                    ));
                                }
                                if new_layout.layer != old_layout.layer {
                                    tasks.push(Task::done(
                                        Message::LayerChange {
                                            id: self.id,
                                            layer: new_layout.layer.into(),
                                        },
                                    ));
                                }
                                if new_layout.gaps != old_layout.gaps {
                                    // let gaps = new_layout.gaps;
                                    // this doesn't work for some reason
                                    // tasks.push(Task::done(
                                    //     Message::MarginChange {
                                    //         id: self.id,
                                    //         margin: (gaps, gaps, gaps, gaps),
                                    //     },
                                    // ));

                                    tasks.push(iced::window::close(self.id));
                                    let (id, open_task) =
                                        open_window(new_layout);
                                    self.id = id;
                                    tasks.push(open_task);
                                }

                                self.config = config;
                                return Task::batch(tasks);
                            }
                            Err(e) => {
                                error!("{e:?}");
                                if let Err(e) = Notification::new()
                                    .summary(BAR_NAMESPACE)
                                    .body("Failed to parse config file")
                                    .show()
                                {
                                    warn!(
                                        "Failed to send config parse error notification: {e:?}"
                                    );
                                }
                            }
                        }
                    }
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
                .map_or_else(iced::Task::none, |cs| {
                    cs.update_gradient(gradient)
                }),
            Message::MprisEvent(event) => self
                .mpris_service
                .as_mut()
                .map_or_else(iced::Task::none, |ms| ms.handle_event(event)),
            Message::MediaControl(control, player) => Task::perform(
                async move {
                    if let Ok(connection) = Connection::session().await
                        && let Ok(player) =
                            PlayerProxy::new(&connection, player).await
                        && let Err(e) = match control {
                            MediaControl::Play => player.play().await,
                            MediaControl::Pause => player.pause().await,
                            MediaControl::PlayPause => {
                                player.play_pause().await
                            }
                            MediaControl::Stop => player.stop().await,
                            MediaControl::Next => player.next().await,
                            MediaControl::Previous => player.previous().await,
                            MediaControl::Seek(amount) => {
                                player.seek(amount).await
                            }
                            MediaControl::Volume(amount) => {
                                match player.volume().await {
                                    Ok(current) => {
                                        player
                                            .set_volume(
                                                (current + amount).max(0.0),
                                            )
                                            .await
                                    }
                                    Err(e) => Err(e),
                                }
                            }
                            MediaControl::SetVolume(amount) => {
                                player.set_volume(amount.max(0.0)).await
                            }
                        }
                    {
                        error!("{e}");
                    }
                },
                |()| Message::NoOp,
            ),
            Message::Command(cmd) => {
                info!("{cmd}");
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
            // Message::AnchorChange { id, anchor } => todo!(),
            // Message::SetInputRegion { id, callback } => todo!(),
            // Message::AnchorSizeChange { id, anchor, size } => todo!(),
            // Message::LayerChange { id, layer } => todo!(),
            // Message::MarginChange { id, margin } => todo!(),
            // Message::SizeChange { id, size } => todo!(),
            // Message::ExclusiveZoneChange { id, zone_size } => todo!(),
            // Message::VirtualKeyboardPressed { time, key } => todo!(),
            // Message::NewLayerShell { settings, id } => todo!(),
            // Message::NewBaseWindow { settings, id } => todo!(),
            // Message::NewPopUp { settings, id } => todo!(),
            // Message::NewMenu { settings, id } => todo!(),
            // Message::NewInputPanel { settings, id } => todo!(),
            // Message::RemoveWindow(id) => todo!(),
            // Message::ForgetLastOutput => todo!(),
            _iced_layershell => unreachable!(),
        }
    }

    fn view_bar(&self) -> Element<'_, Message> {
        let mut start_views: Vec<(Element<Message>, usize)> = vec![];
        let mut middle_views: Vec<(Element<Message>, usize)> = vec![];
        let mut end_views: Vec<(Element<Message>, usize)> = vec![];

        let mut alignments = [
            (BarAlignment::Start, &mut start_views),
            (BarAlignment::Middle, &mut middle_views),
            (BarAlignment::End, &mut end_views),
        ];

        if let Some(service) = &self.battery_service {
            for (pos, target) in &mut alignments {
                target.extend(
                    self.battery_views
                        .iter()
                        .filter(|v| v.position.align == *pos)
                        .map(|v| {
                            (
                                v.view(service, &self.config.layout),
                                v.position.idx,
                            )
                        }),
                );
            }
        }

        if let Some(service) = &self.time_service {
            for (pos, target) in &mut alignments {
                target.extend(
                    self.time_views
                        .iter()
                        .filter(|v| v.position.align == *pos)
                        .map(|v| {
                            (
                                v.view(service, &self.config.layout),
                                v.position.idx,
                            )
                        }),
                );
            }
        }

        if let Some(service) = &self.cava_service {
            for (pos, target) in &mut alignments {
                target.extend(
                    self.cava_views
                        .iter()
                        .filter(|v| v.position.align == *pos)
                        .map(|v| {
                            (
                                v.view(service, &self.config.layout),
                                v.position.idx,
                            )
                        }),
                );
            }
        }

        if let Some(service) = &self.mpris_service {
            for (pos, target) in &mut alignments {
                target.extend(
                    self.mpris_views
                        .iter()
                        .filter(|v| v.position.align == *pos)
                        .map(|v| {
                            (
                                v.view(service, &self.config.layout),
                                v.position.idx,
                            )
                        }),
                );
            }
        }

        if let Some(service) = &self.niri_service {
            for (pos, target) in &mut alignments {
                target.extend(
                    self.niri_views
                        .iter()
                        .filter(|v| v.position.align == *pos)
                        .map(|v| {
                            (
                                v.view(
                                    service,
                                    &self.config.layout,
                                    &self.config.style,
                                ),
                                v.position.idx,
                            )
                        }),
                );
            }
        }

        for (pos, target) in &mut alignments {
            target.extend(
                self.label_views
                    .iter()
                    .filter(|v| v.position.align == *pos)
                    .map(|v| (v.view(&self.config.layout), v.position.idx)),
            );
        }

        let start_views: Vec<Element<Message>> = start_views
            .into_iter()
            .sorted_by_key(|(_, idx)| *idx)
            .map(|(v, _)| v)
            .collect();

        let middle_views: Vec<Element<Message>> = middle_views
            .into_iter()
            .sorted_by_key(|(_, idx)| *idx)
            .map(|(v, _)| v)
            .collect();

        let end_views: Vec<Element<Message>> = end_views
            .into_iter()
            .sorted_by_key(|(_, idx)| *idx)
            .map(|(v, _)| v)
            .collect();

        let vertical = self.config.layout.anchor.vertical();

        let start_section = if vertical {
            Container::new(
                Column::with_children(start_views).align_x(Alignment::Center),
            )
            .align_x(Alignment::Center)
            .align_y(Alignment::Start)
        } else {
            Container::new(
                Row::with_children(start_views)
                    .align_y(Alignment::Center)
                    .padding(left(5).right(5))
                    .spacing(5),
            )
            .align_x(Alignment::Start)
            .align_y(Alignment::Center)
        };

        let start_section =
            start_section.width(Length::Fill).height(Length::Fill);

        let middle_section = if vertical {
            Container::new(
                Column::with_children(middle_views).align_x(Alignment::Center),
            )
        } else {
            Container::new(
                Row::with_children(middle_views)
                    .align_y(Alignment::Center)
                    .spacing(5),
            )
        };

        let middle_section = middle_section
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center);

        let end_section = if vertical {
            Container::new(
                Column::with_children(end_views).align_x(Alignment::Center),
            )
            .align_x(Alignment::Center)
            .align_y(Alignment::End)
        } else {
            Container::new(
                Row::with_children(end_views)
                    .align_y(Alignment::Center)
                    .spacing(5)
                    .padding(left(5).right(5)),
            )
            .align_x(Alignment::End)
            .align_y(Alignment::Center)
        };

        let end_section = end_section.width(Length::Fill).height(Length::Fill);

        let layout = stack![start_section, middle_section, end_section];

        let bar = if vertical {
            Container::new(layout)
                .width(Length::Fixed(self.config.layout.width as f32))
                .height(Length::Fill)
        } else {
            Container::new(layout)
                .width(Length::Fill)
                .height(Length::Fixed(self.config.layout.width as f32))
        };

        bar.style(|_theme| container::Style {
            background: Some(Background::Color(*self.config.style.background)),
            border: rounded(self.config.style.border_radius),
            ..Default::default()
        })
        .into()
    }

    pub fn view(&self, id: Id) -> Element<'_, Message> {
        if id == self.id {
            self.view_bar()
        } else {
            debug!("different id");
            Column::new().into()
        }
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
