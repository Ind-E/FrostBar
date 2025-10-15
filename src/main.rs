use chrono::{DateTime, Local};
use itertools::Itertools;
use tracing::{debug, info, warn};

use tokio::process::Command as TokioCommand;

use iced::{
    Alignment, Background, Color, Element, Event, Length, Pixels, Settings,
    Size, Subscription, Task, Theme,
    border::rounded,
    padding::left,
    theme,
    widget::{Column, Container, Row, container, image, stack},
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
    module::Modules,
    services::{mpris::MprisEvent, niri::NiriEvent},
    utils::{CommandSpec, init_tracing, open_dummy_window, open_window},
    views::BarAlignment,
};

mod config;
mod constants;
mod dbus_proxy;
mod file_watcher;
mod icon_cache;
mod module;
mod services;
mod style;
mod utils;
mod views;

#[cfg(feature = "tracy-allocations")]
#[global_allocator]
static GLOBAL: tracy_client::ProfiledAllocator<std::alloc::System> =
    tracy_client::ProfiledAllocator::new(std::alloc::System, 100);

pub fn main() -> iced::Result {
    #[cfg(feature = "tracy")]
    tracy_client::Client::start();

    iced::daemon(
        || {
            let (config, config_path, config_dir) = Config::init();

            let log_dir = init_tracing(&config_dir);

            info!("starting version {}", env!("CARGO_PKG_VERSION"));
            info!("saving logs to {:?}", log_dir);

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
        id: Some(BAR_NAMESPACE.to_string()),
        fonts: vec![FIRA_CODE_BYTES.into()],
        default_font: FIRA_CODE,
        default_text_size: Pixels(16.0),
        antialiasing: true,
        ..Default::default()
    })
    .run()
}

#[derive(Debug, Clone, Copy)]
pub enum MouseEvent {
    Workspace(u64),
}

#[derive(Debug, Clone)]
pub enum Message {
    IcedEvent(Event),
    MediaControl(MediaControl, String),
    FileWatcherEvent(FileWatcherEvent),

    CavaColorUpdate(Option<Vec<Color>>),
    PlayerArtUpdate(String, Option<(image::Handle, Option<Vec<Color>>)>),

    MouseEntered(MouseEvent),
    MouseExited(MouseEvent),

    Command(CommandSpec),
    ErrorMessage(String),
    NoOp,

    Msg(ModuleMessage),
}

#[derive(Debug, Clone)]
pub enum ModuleMessage {
    Tick(DateTime<Local>),
    UpdateBattery(()),
    Niri(NiriEvent),
    CavaUpdate(Option<String>),
    Mpris(MprisEvent),
}

pub struct Bar {
    id: Option<Id>,
    dummy_id: Id,
    monitor_size: Option<Size>,
    config: Config,
    config_path: PathBuf,

    modules: Modules,
}

#[profiling::all_functions]
impl Bar {
    pub fn new(
        mut config: Config,
        config_path: PathBuf,
    ) -> (Self, Task<Message>) {
        let icon_cache = Arc::new(Mutex::new(IconCache::new()));

        let mut modules =
            Modules::new(icon_cache, config.style.icon_theme.clone());
        modules.update_from_config(&mut config);

        let (dummy_id, open_dummy) = open_dummy_window();

        let bar = Self {
            id: None,
            monitor_size: None,
            dummy_id,
            modules,
            config,
            config_path,
        };

        (bar, Task::batch(vec![open_dummy]))
    }

    fn title(&self, _id: Id) -> String {
        String::from(BAR_NAMESPACE)
    }

    pub fn namespace(&self) -> String {
        String::from(BAR_NAMESPACE)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions: Vec<Subscription<Message>> =
            Vec::with_capacity(8);

        subscriptions.push(iced::event::listen().map(Message::IcedEvent));
        subscriptions.push(watch_file(self.config_path.clone()));
        subscriptions.extend(self.modules.subscriptions());

        Subscription::batch(subscriptions)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::IcedEvent(event) => {
                if let Event::Window(iced::window::Event::Opened {
                    position: _,
                    size,
                }) = event
                    && self.id.is_none()
                {
                    self.monitor_size = Some(size);
                    debug!("measured monitor {size:?}");

                    let (id, open_task) =
                        open_window(&self.config.layout, size);
                    self.id = Some(id);

                    let close_task = iced::window::close(self.dummy_id);
                    return Task::batch([open_task, close_task]);
                }

                if let Event::Window(iced::window::Event::Closed) = event {
                    debug!("window closed");
                }
                Task::none()
            }
            Message::FileWatcherEvent(event) => {
                match event {
                    FileWatcherEvent::Changed => {
                        match Config::load(&self.config_path) {
                            Ok(mut new_config) => {
                                self.modules
                                    .update_from_config(&mut new_config);

                                if self.config.layout == new_config.layout {
                                    self.config = new_config;
                                } else if let Some(id) = self.id {
                                    self.config = new_config;
                                    let close = iced::window::close(id);
                                    let (id, open) = open_window(
                                        &self.config.layout,
                                        self.monitor_size.unwrap(),
                                    );
                                    self.id = Some(id);
                                    return Task::batch([close, open]);
                                }
                            }
                            Err(e) => {
                                error!("{:?}", e);
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
            Message::Msg(module_msg) => self.modules.handle_event(module_msg),
            Message::CavaColorUpdate(gradient) => {
                self.modules.handle_cava_color_update(gradient)
            }
            Message::PlayerArtUpdate(name, art) => {
                self.modules.handle_async_mpris_art_update(&name, art)
            }
            Message::MouseEntered(event) => {
                self.modules.handle_mouse_entered(event)
            }
            Message::MouseExited(event) => {
                self.modules.handle_mouse_exited(event)
            }
            Message::ErrorMessage(msg) => {
                error!("error message: {}", msg);
                Task::none()
            }
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
            Message::Command(cmd) => Task::future(async move {
                let mut command = TokioCommand::new(&cmd.command);
                if let Some(ref args) = cmd.args {
                    command.args(args);
                }

                match command.output().await {
                    Ok(output) => {
                        info!(target: "process", "spawned `{cmd}`");

                        if !output.stdout.is_empty() {
                            info!(target: "process",
                                "{}",
                                String::from_utf8_lossy(&output.stdout)
                            );
                        }

                        if !output.stderr.is_empty() {
                            error!(
                                target: "process",
                                "{}",
                                String::from_utf8_lossy(&output.stderr)
                            );
                        }
                    }

                    Err(e) => {
                        error!(target: "process", "failed to spawn `{cmd}`: {e}");
                    }
                }

                Message::NoOp
            }),

            Message::NoOp => Task::none(),
        }
    }

    fn view_bar(&self) -> Element<'_, Message> {
        let mut start_views: Vec<(Element<Message>, usize)> = vec![];
        let mut middle_views: Vec<(Element<Message>, usize)> = vec![];
        let mut end_views: Vec<(Element<Message>, usize)> = vec![];

        for (element, position) in
            self.modules.render_views(&self.config.layout)
        {
            match position.align {
                BarAlignment::Start => {
                    start_views.push((element, position.idx));
                }
                BarAlignment::Middle => {
                    middle_views.push((element, position.idx));
                }
                BarAlignment::End => end_views.push((element, position.idx)),
            }
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
        if Some(id) == self.id {
            self.view_bar()
        } else {
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
