use chrono::{DateTime, Local};
use iced::{
    Color, Element, Event, Length, Point, Rectangle, Settings, Size, Subscription, Task,
    Theme,
    advanced::{mouse, subscription},
    alignment::{Horizontal, Vertical},
    event,
    padding::top,
    theme,
    time::{self, Duration},
    widget::{Column, Container, Stack, Text, column, container, stack, text},
    window::{
        Id,
        settings::{
            Anchor, KeyboardInteractivity, Layer, LayerShellSettings, PlatformSpecific,
        },
    },
};
use std::{
    collections::HashMap,
    process::Command,
    sync::{Arc, Mutex},
    thread,
};
use zbus::Connection;

use crate::{
    config::{
        BAR_NAMESPACE, BAR_WIDTH, FIRA_CODE, FIRA_CODE_BYTES, GAPS, TOOLTIP_RETRIES,
    },
    dbus_proxy::PlayerProxy,
    icon_cache::IconCache,
    modules::{
        battery::BatteryModule,
        cava::{CavaError, CavaEvents, CavaModule, write_temp_cava_config},
        mpris::{MprisEvent, MprisListener, MprisModule},
        niri::{NiriModule, NiriOutput, NiriSubscriptionRecipe},
        time::TimeModule,
    },
    style::{rounded_corners, tooltip_style},
    tooltip::{Tooltip, TooltipState},
};

mod config;
mod dbus_proxy;
mod icon_cache;
mod modules;
mod style;
mod tooltip;

pub fn main() -> iced::Result {
    pretty_env_logger::init();
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
    Window(u64),
    MprisPlayer(String),
    Tooltip(container::Id),
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
    TooltipMeasured(container::Id, Option<Rectangle>),

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

    tooltips: HashMap<container::Id, Tooltip>,
    tooltip_canvas: Id,
}

impl Bar {
    pub fn new() -> (Self, Task<Message>) {
        let time_module = TimeModule::new();
        let battery_module = BatteryModule::new();

        let mut tooltips = HashMap::new();
        tooltips.insert(battery_module.id.clone(), Tooltip::default());
        tooltips.insert(time_module.id.clone(), Tooltip::default());

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
                    anchor: Some(Anchor::LEFT | Anchor::TOP | Anchor::BOTTOM),
                    exclusive_zone: Some(BAR_WIDTH as i32),
                    margin: Some((GAPS, GAPS, GAPS, GAPS)),
                    input_region: None,
                    keyboard_interactivity: Some(KeyboardInteractivity::None),
                    namespace: Some(String::from(BAR_NAMESPACE)),
                    ..Default::default()
                },
                ..Default::default()
            },
            exit_on_close_request: false,
            ..Default::default()
        });

        let (tooltip_canvas, open_tooltip_canvas) =
            iced::window::open(iced::window::Settings {
                size: Size::new(10.0, 10.0),
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
                        input_region: Some((0, 0, 0, 0)),
                        keyboard_interactivity: Some(KeyboardInteractivity::None),
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
                tooltips,
                tooltip_canvas,
                time_module,
                battery_module,
                niri_module: NiriModule::new(icon_cache.clone()),
                mpris_module: MprisModule::new(),
                cava_module: CavaModule::new(),
            },
            Task::batch(vec![
                open.map(Message::OpenWindow),
                open_tooltip_canvas.map(Message::OpenWindow),
            ]),
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
                if id != self.id && id != self.tooltip_canvas {
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
                    MouseEvent::MprisPlayer(name) => 'block: {
                        let Some(player) = self.mpris_module.players.get(&name) else {
                            break 'block;
                        };
                        let id = player.id.clone();
                        let Some(tooltip) = self.tooltips.get_mut(&id) else {
                            break 'block;
                        };
                        if tooltip.state == TooltipState::Hidden {
                            tooltip.content = Some(player.tooltip());
                            tooltip.state = TooltipState::Measuring(0);

                            return container::visible_bounds(id.clone()).map(
                                move |rect| Message::TooltipMeasured(id.clone(), rect),
                            );
                        }
                    }
                    MouseEvent::Window(w_id) => 'block: {
                        let Some(window) = self.niri_module.windows.get(&w_id) else {
                            break 'block;
                        };
                        let id = window.container_id.clone();
                        let tooltip = self
                            .tooltips
                            .entry(id.clone())
                            .or_insert_with(|| Tooltip::default());

                        if tooltip.state == TooltipState::Hidden
                            && let Some(window) = self
                                .niri_module
                                .windows
                                .values()
                                .find(|w| w.inner.id == w_id)
                        {
                            tooltip.content = window.inner.title.clone();
                            tooltip.state = TooltipState::Measuring(0);

                            return container::visible_bounds(id.clone()).map(
                                move |rect| Message::TooltipMeasured(id.clone(), rect),
                            );
                        }
                    }
                    MouseEvent::Workspace(id) => {
                        self.niri_module.hovered_workspace_id = Some(id);
                    }
                    MouseEvent::Tooltip(id) => {
                        if let Some(tooltip) = self.tooltips.get_mut(&id)
                            && tooltip.state == TooltipState::Hidden
                        {
                            tooltip.content = if id == self.battery_module.id {
                                Some(self.battery_module.tooltip())
                            } else if id == self.time_module.id {
                                Some(self.time_module.tooltip())
                            } else {
                                unreachable!()
                            };
                            tooltip.state = TooltipState::Measuring(0);
                            return container::visible_bounds(id.clone()).map(
                                move |rect| Message::TooltipMeasured(id.clone(), rect),
                            );
                        }
                    }
                };

                Task::none()
            }
            Message::TooltipMeasured(id, rect) => {
                if let Some(tooltip) = self.tooltips.get_mut(&id)
                    && let TooltipState::Measuring(retries) = tooltip.state
                {
                    tooltip.position = match rect {
                        Some(rect) => Some(Point::new(0.0, rect.center().y)),
                        Option::None => {
                            tooltip.state = TooltipState::Measuring(retries + 1);
                            if retries < TOOLTIP_RETRIES {
                                return container::visible_bounds(id.clone()).map(
                                    move |rect| {
                                        Message::TooltipMeasured(id.clone(), rect)
                                    },
                                );
                            } else {
                                None
                            }
                        }
                    };
                    tooltip.state = TooltipState::Visible;
                }
                Task::none()
            }
            Message::MouseExitedBar => {
                self.tooltips.values_mut().for_each(|tip| {
                    tip.state = TooltipState::Hidden;
                });
                self.niri_module.hovered_workspace_id = None;
                Task::none()
            }
            Message::MouseExited(event) => {
                match event {
                    MouseEvent::MprisPlayer(name) => {
                        if let Some(id) = self
                            .mpris_module
                            .players
                            .get(&name)
                            .and_then(|p| Some(&p.id))
                        {
                            let tooltip = self
                                .tooltips
                                .entry(id.clone())
                                .or_insert_with(|| Tooltip::default());
                            tooltip.state = TooltipState::Hidden;
                        }
                    }
                    MouseEvent::Window(w_id) => {
                        if let Some(id) = self
                            .niri_module
                            .windows
                            .get(&w_id)
                            .and_then(|w| Some(&w.container_id))
                        {
                            let tooltip = self
                                .tooltips
                                .entry(id.clone())
                                .or_insert_with(|| Tooltip::default());
                            tooltip.state = TooltipState::Hidden;
                        }
                    }
                    MouseEvent::Workspace(..) => {
                        self.niri_module.hovered_workspace_id = None;
                    }
                    MouseEvent::Tooltip(id) => {
                        if let Some(tooltip) = self.tooltips.get_mut(&id) {
                            tooltip.state = TooltipState::Hidden;
                        }
                    }
                };

                Task::none()
            }
            Message::ErrorMessage(msg) => {
                log::error!("error message: {}", msg);
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
                        log::error!("{e}");
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
                        log::error!("{e}");
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

    fn view_canvas(&self) -> Element<Message> {
        Container::new(
            self.tooltips
                .iter()
                .filter(|(_id, tooltip)| tooltip.state != TooltipState::Hidden)
                .fold(Stack::new(), |stack, (_id, tooltip)| {
                    let Some(content) = tooltip.content.as_ref() else {
                        return stack;
                    };

                    let is_measuring =
                        matches!(tooltip.state, TooltipState::Measuring(_));

                    let (text_color, style) =
                        if is_measuring || tooltip.position.is_none() {
                            (Some(Color::TRANSPARENT), tooltip_style(0.0))
                        } else {
                            (None, tooltip_style(1.0))
                        };

                    let position = tooltip.position.unwrap_or(Point::new(0.0, 0.0));

                    const TOOLTIP_PADDING: u16 = 7;
                    const TEXT_SIZE: u32 = 16;

                    let content_column = content
                        .lines()
                        .map(|line| {
                            Container::new(
                                Text::new(line)
                                    .size(TEXT_SIZE)
                                    .color_maybe(text_color)
                                    .line_height(1.0)
                                    .shaping(text::Shaping::Advanced),
                            )
                            .width(Length::Shrink)
                            .height(TEXT_SIZE)
                            .clip(true)
                        })
                        .fold(Column::new(), |col, text| col.push(text));

                    let y_offset = position.y
                        - content.lines().count() as f32 * (TEXT_SIZE as f32 / 2.0)
                        - TOOLTIP_PADDING as f32;

                    let widget = Container::new(
                        Container::new(content_column)
                            .style(style)
                            .padding(TOOLTIP_PADDING)
                            .width(Length::Shrink)
                            .clip(true),
                    )
                    .padding(top(y_offset).left(position.x));

                    stack.push(widget)
                }),
        )
        .into()
    }

    pub fn view(&self, id: Id) -> Element<Message> {
        if id == self.tooltip_canvas {
            return self.view_canvas();
        } else {
            self.view_bar()
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
