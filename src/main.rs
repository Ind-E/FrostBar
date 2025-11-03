use chrono::Local;
use itertools::Itertools;
use tracing::{debug, info, warn};

use tokio::process::Command as TokioCommand;

use iced::{
    Alignment, Background, Color, Element, Event, Length, Pixels, Rectangle,
    Settings, Size, Subscription, Task, Theme,
    advanced::subscription::from_recipe,
    border::rounded,
    padding::{left, top},
    theme,
    widget::{Column, Container, Row, container, stack},
    window::Id,
};
use notify_rust::Notification;
use std::time::Duration;
use tracing_subscriber::{
    EnvFilter, fmt, layer::SubscriberExt, reload, util::SubscriberInitExt,
};
use zbus::Connection;

use tracing::error;

use crate::{
    config::{ColorVars, RawConfig, Config, MediaControl},
    constants::{BAR_NAMESPACE, FIRA_CODE, FIRA_CODE_BYTES},
    dbus_proxy::PlayerProxy,
    file_watcher::{CheckResult, CheckType, ConfigPath, watch_config},
    icon_cache::IconCache,
    module::{ModuleAction, Modules},
    services::{
        cava::CavaSubscriptionRecipe, mpris::MprisService, niri::NiriService,
        systray::Systray,
    },
    utils::{
        CommandSpec, init_tracing, open_dummy_window, open_tooltip_window,
        open_window,
    },
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
            let debug = cfg!(debug_assertions);

            let filter =
                EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                    if debug {
                        EnvFilter::new("info,frostbar=debug")
                    } else {
                        EnvFilter::new("error,frostbar=info")
                    }
                });

            let stderr_layer =
                fmt::layer().compact().with_writer(std::io::stderr);

            let (file_layer, handle) = reload::Layer::new(None);

            tracing_subscriber::registry()
                .with(filter)
                .with(stderr_layer)
                .with(file_layer)
                .init();

            let (config, color_vars, config_path, config_dir) = RawConfig::init();

            let logfile_path = init_tracing(&config_dir, &handle);

            info!("starting version {}", env!("CARGO_PKG_VERSION"));
            info!("saving logs to {:?}", logfile_path);

            Bar::new(config, color_vars, config_path)
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
        antialiasing: false,
        ..Default::default()
    })
    .run()
}

#[derive(Debug, Clone, Copy)]
pub enum MouseEvent {
    Workspace(u64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TooltipId {
    pub id: container::Id,
    pub bounds: Option<Rectangle>,
}

#[derive(Debug, Clone)]
pub enum Message {
    IcedEvent(Event),
    MediaControl(MediaControl, String),
    FileWatcherEvent(CheckResult),

    Command(CommandSpec),
    NoOp,

    OpenTooltip(container::Id),
    TooltipPositionMeasured(TooltipId),
    CloseTooltip(container::Id),

    Module(module::Message),
}

pub struct Bar {
    id: Option<Id>,
    dummy_id: Id,
    monitor_size: Option<Size>,
    config: Config,
    color_vars: ColorVars,
    path: ConfigPath,

    modules: Modules,

    tooltip_window_id: Option<Id>,
    active_tooltip_id: Option<TooltipId>,
}

#[profiling::all_functions]
impl Bar {
    pub fn new(
        mut config: Config,
        color_vars: ColorVars,
        path: ConfigPath,
    ) -> (Self, Task<Message>) {
        let icon_cache = IconCache::new();

        let mut modules = Modules::new(icon_cache);
        modules.update_from_config(&mut config);

        let (dummy_id, open_dummy) = open_dummy_window();

        let bar = Self {
            id: None,
            monitor_size: None,
            dummy_id,
            modules,
            config,
            color_vars,
            path,
            tooltip_window_id: None,
            active_tooltip_id: None,
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
        subscriptions.push(watch_config(self.path.clone()));

        subscriptions.push(
            iced::time::every(Duration::from_secs(1))
                .map(|_| Message::Module(module::Message::Tick(Local::now()))),
        );

        subscriptions.push(
            from_recipe(CavaSubscriptionRecipe {})
                .map(|f| Message::Module(module::Message::CavaUpdate(f))),
        );

        subscriptions.push(MprisService::subscription());
        subscriptions.push(NiriService::subscription());

        subscriptions.push(Systray::subscription());

        // subscriptions.extend(self.modules.subscriptions());

        Subscription::batch(subscriptions)
    }

    fn reload_config(&mut self) -> Task<Message> {
        match RawConfig::load(&self.path.config) {
            Ok(new_config) => {
                let mut new_config = new_config.hydrate(&self.color_vars);
                self.modules.update_from_config(&mut new_config);

                if self.config.layout == new_config.layout {
                    self.config = new_config;
                    return Task::done(Message::Module(
                        module::Message::SynchronizeAll,
                    ));
                } else if let Some(id) = self.id {
                    self.config = new_config;
                    let close = iced::window::close(id);
                    let (id, open) = open_window(
                        &self.config.layout,
                        self.monitor_size.unwrap(),
                    );
                    self.id = Some(id);
                    return Task::batch([
                        close,
                        open,
                        Task::done(Message::Module(
                            module::Message::SynchronizeAll,
                        )),
                    ]);
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
        Task::none()
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

                // if let Event::Window(iced::window::Event::Closed) = event {
                //     debug!("window closed");
                // }
            }
            Message::OpenTooltip(id) => {
                return container::visible_bounds(id.clone()).map(
                    move |bounds| {
                        Message::TooltipPositionMeasured(TooltipId {
                            id: id.clone(),
                            bounds,
                        })
                    },
                );
            }
            Message::TooltipPositionMeasured(tooltip_id) => {
                let old_id = self.tooltip_window_id.take();

                let (win_id, open_task) = open_tooltip_window();
                self.tooltip_window_id = Some(win_id);
                self.active_tooltip_id = Some(tooltip_id);

                if let Some(old_id) = old_id {
                    debug!(
                        "opening tooltip {}, closing tooltip {}",
                        self.tooltip_window_id.unwrap(),
                        old_id
                    );
                    return open_task.chain(iced::window::close(old_id));
                }

                debug!("opening tooltip {}", self.tooltip_window_id.unwrap());
                return open_task;
            }
            Message::CloseTooltip(id) => {
                if self.active_tooltip_id.as_ref().is_some_and(|t| t.id == id)
                    && let Some(window_id) = self.tooltip_window_id.take()
                {
                    debug!("closing tooltip {}", window_id);
                    self.active_tooltip_id = None;
                    return iced::window::close(window_id);
                }
            }
            Message::FileWatcherEvent(event) => {
                match event.colors {
                    CheckType::Changed => {
                        match ColorVars::load(&self.path.colors) {
                            Ok(new_color_vars) => {
                                self.color_vars = new_color_vars;
                                return self.reload_config();
                            }
                            Err(e) => {
                                error!("{:?}", e);
                                if let Err(e) = Notification::new()
                                    .summary(BAR_NAMESPACE)
                                    .body("Failed to parse colors file")
                                    .show()
                                {
                                    warn!(
                                        "Failed to send colors parse error notification: {e:?}"
                                    );
                                }
                            }
                        }
                    }
                    CheckType::Missing => {
                        if let Err(e) = Notification::new()
                            .summary(&format!(
                                "Colors file not found at {}",
                                self.path.config.display()
                            ))
                            .show()
                        {
                            warn!(
                                "Failed to send colors parse error notification: {e:?}"
                            );
                        }
                    }
                    CheckType::Unchanged => {}
                }

                match event.config {
                    CheckType::Changed => {
                        return self.reload_config();
                    }
                    CheckType::Missing => {
                        if let Err(e) = Notification::new()
                            .summary(&format!(
                                "Config file not found at {}",
                                self.path.config.display()
                            ))
                            .show()
                        {
                            warn!(
                                "Failed to send config parse error notification: {e:?}"
                            );
                        }
                    }
                    CheckType::Unchanged => {}
                }
            }
            Message::Module(module_msg) => {
                match self.modules.update(module_msg) {
                    ModuleAction::Task(task) => {
                        return task.map(Message::Module);
                    }
                    ModuleAction::None => {}
                }
            }
            Message::MediaControl(control, player) => {
                return Task::perform(
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
                                MediaControl::Previous => {
                                    player.previous().await
                                }
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
                );
            }
            Message::Command(cmd) => {
                return Task::future(async move {
                    let mut command = TokioCommand::new(&cmd.command);
                    if let Some(ref args) = cmd.args {
                        command.args(args);
                    }

                    match command.output().await {
                        Ok(output) => {
                            info!(target: "child_process", "spawned `{cmd}`");

                            if !output.stdout.is_empty() {
                                info!(target: "child_process",
                                    "{}",
                                    String::from_utf8_lossy(&output.stdout)
                                );
                            }

                            if !output.stderr.is_empty() {
                                error!(
                                    target: "child_process",
                                    "{cmd}: {}",
                                    String::from_utf8_lossy(&output.stderr)
                                );
                            }
                        }

                        Err(e) => {
                            error!(target: "child_process", "failed to spawn `{cmd}`: {e}");
                        }
                    }

                    Message::NoOp
                });
            }

            Message::NoOp => {}
        }

        Task::none()
    }

    #[inline(always)]
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
                Column::with_children(start_views)
                    .align_x(Alignment::Center)
                    .padding(top(5).bottom(5)),
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
                Column::with_children(end_views)
                    .align_x(Alignment::Center)
                    .padding(top(5).bottom(5)),
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
            background: Some(Background::Color(self.config.style.background)),
            border: rounded(self.config.style.border_radius),
            ..Default::default()
        })
        .into()
    }

    #[inline(always)]
    fn view_tooltip(&self, tooltip_id: &TooltipId) -> Element<'_, Message> {
        let content = self
            .modules
            .render_tooltip_for_id(&tooltip_id.id)
            .unwrap_or_else(|| Column::new().into());

        let bounds = tooltip_id.bounds.unwrap_or_default();
        let mut container =
            Container::new(content).padding(5).style(|_theme: &Theme| {
                container::Style {
                    background: Some(Background::Color(
                        self.config.style.background,
                    )),
                    border: rounded(self.config.style.border_radius),
                    ..Default::default()
                }
            });
        match self.config.layout.anchor {
            config::Anchor::Right => {
                container = Container::new(container).align_right(Length::Fill);
            }
            config::Anchor::Bottom => {
                container =
                    Container::new(container).align_bottom(Length::Fill);
            }
            config::Anchor::Top | config::Anchor::Left => {}
        }

        let pin = iced::widget::pin(container);
        if self.config.layout.anchor.vertical() {
            pin.y(bounds.y).into()
        } else {
            pin.x(bounds.x).into()
        }
    }

    pub fn view(&self, id: Id) -> Element<'_, Message> {
        if Some(id) == self.id {
            self.view_bar()
        } else if Some(id) == self.tooltip_window_id
            && let Some(tooltip_id) = &self.active_tooltip_id
        {
            self.view_tooltip(tooltip_id)
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
