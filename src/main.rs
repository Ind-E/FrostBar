use clap::Parser;
use iced::{
    Alignment, Background, Color, Event, Font, Length, Pixels, Rectangle,
    Subscription, Task, Theme,
    border::rounded,
    font::{Family, Weight},
    padding::{left, top},
    theme,
    widget::{
        self, Column, Container, MouseArea, Row, container, selector::Target,
        stack,
    },
    window::Id,
};
use iced_layershell::settings::{LayerShellSettings, StartMode};
use itertools::Itertools;
use smol::process::Command as TokioCommand;
use tracing::{debug, error, info};
use tracing_subscriber::Layer;
use tracing_subscriber::{
    fmt::{self},
    layer::SubscriberExt,
    reload,
    util::SubscriberInitExt,
};
use zbus::Connection;

use crate::{
    cli::{Cli, handle_subcommand},
    config::{Anchor, ColorVars, Config, MediaControl, RawConfig},
    file_watcher::{CheckResult, CheckType, ConfigPath, watch_config},
    icon_cache::IconCache,
    modules::{
        BarAlignment,
        CommandSpec,
        ModuleAction,
        ModuleMsg,
        Modules,
        mpris::mpris_player::PlayerProxy,
        // system_tray::service::SystemTrayService,
    },
    utils::{
        log::{LogManager, get_default_filter, notification},
        window::{open_tooltip_window, open_window},
    },
};

mod cli;
mod config;
mod file_watcher;
mod icon_cache;
mod modules;
mod utils;

type Element<'a> = iced::Element<'a, Message>;

pub const FIRA_CODE_BYTES: &[u8] =
    include_bytes!("../assets/FiraCodeNerdFontMono-Medium.ttf");
pub const FIRA_CODE: Font = Font {
    family: Family::Name("FiraCode Nerd Font Mono"),
    weight: Weight::Medium,
    ..Font::DEFAULT
};

pub const BAR_NAMESPACE: &str = "FrostBar";

#[cfg(feature = "tracy-allocations")]
#[global_allocator]
static GLOBAL: tracy_client::ProfiledAllocator<std::alloc::System> =
    tracy_client::ProfiledAllocator::new(std::alloc::System, 100);

#[cfg(not(feature = "tracy-allocations"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

pub fn main() -> Result<(), iced_layershell::Error> {
    #[cfg(feature = "tracy")]
    tracy_client::Client::start();

    iced_layershell::daemon(
        || {
            let cli = Cli::parse();

            let log_manager = LogManager::init();

            if let Some(sub) = cli.subcommand {
                handle_subcommand(sub, &log_manager);
            }

            let stderr_layer = fmt::layer()
                .compact()
                .with_writer(std::io::stderr)
                .with_line_number(true)
                .with_filter(get_default_filter());

            let (file_layer, handle) = reload::Layer::new(None);
            let file_layer = file_layer.with_filter(get_default_filter());

            let registry = tracing_subscriber::registry()
                .with(stderr_layer)
                .with(file_layer);

            registry.init();

            let (config, color_vars, config_path) =
                RawConfig::init(cli.config_dir);

            let logfile_path = log_manager.setup_logging(&handle);

            info!("starting version {}", env!("CARGO_PKG_VERSION"));
            info!("saving logs to {:?}", logfile_path);

            Bar::new(config, color_vars, config_path)
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
    pub id: widget::Id,
    pub bounds: Option<Rectangle>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MenuId {
    pub id: widget::Id,
    pub bounds: Option<Rectangle>,
}

#[iced_layershell::to_layer_message(multi)]
#[derive(Debug, Clone)]
pub enum Message {
    IcedEvent(Event),
    MediaControl(MediaControl, String),
    FileWatcherEvent(CheckResult),

    Command(CommandSpec),
    NoOp,

    OpenTooltip(widget::Id),
    TooltipPositionMeasured(TooltipId),
    CloseTooltip(widget::Id),

    // OpenMenu(widget::Id),
    // ActivateMenu(String),
    // MenuPositionMeasured(MenuId),
    // CloseMenu(widget::Id),
    Module(ModuleMsg),
}

pub struct Bar {
    id: Id,
    config: Config,
    color_vars: ColorVars,
    path: ConfigPath,

    modules: Modules,
    icon_cache: IconCache,

    tooltip_window_id: Option<Id>,
    active_tooltip_id: Option<TooltipId>,

    menu_window_id: Option<Id>,
    active_menu_id: Option<MenuId>,
}

#[profiling::all_functions]
impl Bar {
    pub fn new(
        mut config: Config,
        color_vars: ColorVars,
        path: ConfigPath,
    ) -> (Self, Task<Message>) {
        let icon_cache = IconCache::new();

        let mut modules = Modules::new();
        modules.update_from_config(&mut config, &icon_cache);

        let (id, open_task) = open_window(&config.layout);

        let bar = Self {
            id,
            modules,
            icon_cache,
            config,
            color_vars,
            path,
            tooltip_window_id: None,
            active_tooltip_id: None,
            menu_window_id: None,
            active_menu_id: None,
        };

        (bar, open_task)
    }

    fn title(&self, id: Id) -> Option<String> {
        if self.id == id {
            Some(String::from(BAR_NAMESPACE))
        } else {
            None
        }
    }

    pub fn namespace() -> String {
        String::from(BAR_NAMESPACE)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let iced_event_sub = iced::event::listen().map(Message::IcedEvent);
        let watch_config_sub = watch_config(self.path.clone());
        let modules_sub = self.modules.subscriptions();
        Subscription::batch([iced_event_sub, watch_config_sub, modules_sub])
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        #[cfg(feature = "tracy")]
        let _span = tracy_client::span!("iced_update");
        match message {
            Message::IcedEvent(event) => {
                // if let Event::Window(iced::window::Event::Opened {
                //     position: _,
                //     size,
                // }) = event
                //     && self.id.is_none()
                // {
                //     self.monitor_size = Some(size);
                //     debug!("measured monitor {size:?}");
                //
                //     let (id, open_task) = open_window(&self.config.layout);
                //     self.id = Some(id);
                //
                //     let close_task = iced::window::close(self.dummy_id);
                //     return Task::batch([open_task, close_task]);
                // }

                if let Event::Mouse(iced::mouse::Event::ButtonPressed(_)) =
                    event
                    && let Some(window_id) = self.menu_window_id.take()
                {
                    debug!("closing menu {}", window_id);
                    self.active_menu_id = None;
                    return iced::window::close(window_id);
                }

                // if let Event::Window(iced::window::Event::Closed) = event {
                //     debug!("window closed");
                // }
            }
            // Message::ActivateMenu(address) => {
            //     debug!("Activating menu {address}");
            //     if let Some(task) = self.modules.systray.activate_menu(address)
            //     {
            //         return task;
            //     }
            // }
            // Message::OpenMenu(id) => {
            //     return container::visible_bounds(id.clone()).map(
            //         move |bounds| {
            //             Message::MenuPositionMeasured(MenuId {
            //                 id: id.clone(),
            //                 bounds,
            //             })
            //         },
            //     );
            // }
            // Message::MenuPositionMeasured(menu_id) => {
            //     let old_id = self.menu_window_id.take();
            //
            //     let (win_id, open_task) = open_tooltip_window();
            //     self.menu_window_id = Some(win_id);
            //     self.active_menu_id = Some(menu_id);
            //
            //     if let Some(old_id) = old_id {
            //         debug!(
            //             "opening menu {}, closing menu {}",
            //             self.menu_window_id.unwrap(),
            //             old_id
            //         );
            //         return open_task.chain(iced::window::close(old_id));
            //     }
            //
            //     debug!("opening menu {}", self.menu_window_id.unwrap());
            //     return open_task;
            // }
            // Message::CloseMenu(id) => {
            //     if self.active_menu_id.as_ref().is_some_and(|t| t.id == id)
            //         && let Some(window_id) = self.menu_window_id.take()
            //     {
            //         debug!("closing menu {}", window_id);
            //         self.active_menu_id = None;
            //         return iced::window::close(window_id);
            //     }
            // }
            Message::OpenTooltip(id) => {
                return widget::selector::find(id.clone()).map(move |target| {
                    if let Some(Target::Container { visible_bounds, .. }) =
                        target
                    {
                        Message::TooltipPositionMeasured(TooltipId {
                            id: id.clone(),
                            bounds: visible_bounds,
                        })
                    } else {
                        error!("failed to find tooltip");
                        Message::NoOp
                    }
                });
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
                                notification(
                                    "Failed to parse colors file\nrun `frostbar validate` to see the errors",
                                );
                            }
                        }
                    }
                    CheckType::Disappeared => {
                        notification(&format!(
                            "Colors file not found at {}",
                            self.path.colors.display()
                        ));
                    }
                    CheckType::Missing | CheckType::Unchanged => {}
                }
                match event.config {
                    CheckType::Changed => {
                        return self.reload_config();
                    }
                    CheckType::Disappeared => {
                        notification(&format!(
                            "Config file not found at {}",
                            self.path.config.display()
                        ));
                    }
                    CheckType::Missing | CheckType::Unchanged => {}
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
                                                    (current
                                                        + f64::from(amount))
                                                    .max(0.0),
                                                )
                                                .await
                                        }
                                        Err(e) => Err(e),
                                    }
                                }

                                MediaControl::SetVolume(amount) => {
                                    player
                                        .set_volume(f64::from(amount.max(0.0)))
                                        .await
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
                            info!("spawned `{cmd}`");

                            if !output.stdout.is_empty() {
                                info!(
                                    "{cmd}: {}",
                                    String::from_utf8_lossy(&output.stdout)
                                );
                            }

                            if !output.stderr.is_empty() {
                                error!(
                                    "child process {cmd}: {}",
                                    String::from_utf8_lossy(&output.stderr)
                                );
                            }
                        }

                        Err(e) => {
                            error!("failed to spawn `{cmd}`: {e}");
                        }
                    }

                    Message::NoOp
                });
            }

            Message::NoOp => {}

            Message::AnchorChange { .. }
            | Message::SetInputRegion { .. }
            | Message::AnchorSizeChange { .. }
            | Message::LayerChange { .. }
            | Message::MarginChange { .. }
            | Message::SizeChange { .. }
            | Message::ExclusiveZoneChange { .. }
            | Message::VirtualKeyboardPressed { .. }
            | Message::NewLayerShell { .. }
            | Message::NewBaseWindow { .. }
            | Message::NewPopUp { .. }
            | Message::NewMenu { .. }
            | Message::NewInputPanel { .. }
            | Message::RemoveWindow(_)
            | Message::ForgetLastOutput => unreachable!(),
        }

        Task::none()
    }

    #[inline(always)]
    fn view_bar(&self) -> Element<'_> {
        let mut start_views: Vec<(Element, usize)> = vec![];
        let mut middle_views: Vec<(Element, usize)> = vec![];
        let mut end_views: Vec<(Element, usize)> = vec![];

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

        let start_views: Vec<Element> = start_views
            .into_iter()
            .sorted_unstable_by_key(|(_, idx)| *idx)
            .map(|(v, _)| v)
            .collect();

        let middle_views: Vec<Element> = middle_views
            .into_iter()
            .sorted_unstable_by_key(|(_, idx)| *idx)
            .map(|(v, _)| v)
            .collect();

        let end_views: Vec<Element> = end_views
            .into_iter()
            .sorted_unstable_by_key(|(_, idx)| *idx)
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
    fn view_tooltip<'a>(&'a self, tooltip_id: &'a TooltipId) -> Element<'a> {
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
            Anchor::Right => {
                container = Container::new(container).align_right(Length::Fill);
            }
            Anchor::Bottom => {
                container =
                    Container::new(container).align_bottom(Length::Fill);
            }
            Anchor::Top | Anchor::Left => {}
        }

        let pin = if self.config.layout.anchor.vertical() {
            iced::widget::pin(container).y(bounds.y)
        } else {
            iced::widget::pin(container).x(bounds.x)
        };

        MouseArea::new(
            Container::new(pin).width(Length::Fill).height(Length::Fill),
        )
        .on_move(|_| Message::CloseTooltip(tooltip_id.id.clone()))
        .on_press(Message::CloseTooltip(tooltip_id.id.clone()))
        .into()
    }

    // #[inline(always)]
    // fn view_menu(&self, menu_id: &MenuId) -> Element<'_> {
    //     let content = self
    //         .modules
    //         .render_menu_for_id(&menu_id.id)
    //         .unwrap_or_else(|| Column::new().into());
    //
    //     let bounds = menu_id.bounds.unwrap_or_default();
    //     let mut container =
    //         Container::new(content).padding(5).style(|_theme: &Theme| {
    //             container::Style {
    //                 background: Some(Background::Color(
    //                     self.config.style.background,
    //                 )),
    //                 border: rounded(self.config.style.border_radius),
    //                 ..Default::default()
    //             }
    //         });
    //     match self.config.layout.anchor {
    //         Anchor::Right => {
    //             container = Container::new(container).align_right(Length::Fill);
    //         }
    //         Anchor::Bottom => {
    //             container =
    //                 Container::new(container).align_bottom(Length::Fill);
    //         }
    //         Anchor::Top | Anchor::Left => {}
    //     }
    //
    //     let pin = if self.config.layout.anchor.vertical() {
    //         iced::widget::pin(container).y(bounds.y)
    //     } else {
    //         iced::widget::pin(container).x(bounds.x)
    //     };
    //
    //     MouseArea::new(
    //         Container::new(pin).width(Length::Fill).height(Length::Fill),
    //     )
    //     .on_press(Message::CloseMenu(menu_id.id.clone()))
    //     .into()
    // }

    pub fn view(&self, id: Id) -> Element<'_> {
        #[cfg(feature = "tracy")]
        tracy_client::frame_mark();
        if id == self.id {
            self.view_bar()
        } else if Some(id) == self.tooltip_window_id
            && let Some(tooltip_id) = &self.active_tooltip_id
        {
            self.view_tooltip(tooltip_id)
        }
        // else if Some(id) == self.menu_window_id
        //     && let Some(menu_id) = &self.active_menu_id
        // {
        //     self.view_menu(menu_id)
        // }
        else {
            debug!("viewing nothing??");
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

    fn reload_config(&mut self) -> Task<Message> {
        match RawConfig::load(&self.path.config) {
            Ok(new_config) => {
                let mut new_config = new_config.hydrate(&self.color_vars);
                self.modules
                    .update_from_config(&mut new_config, &self.icon_cache);

                if self.config.layout == new_config.layout {
                    self.config = new_config;
                    self.modules.synchronize_views();
                } else {
                    self.config = new_config;
                    let close = iced::window::close(self.id);
                    let (id, open) = open_window(&self.config.layout);
                    self.id = id;
                    self.modules.synchronize_views();
                    return Task::batch([close, open]);
                }
            }
            Err(e) => {
                error!("{:?}", e);
                notification(
                    "Failed to parse config file\nrun `frostbar validate` to see the errors",
                );
            }
        }
        Task::none()
    }
}
