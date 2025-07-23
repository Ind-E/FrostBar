use chrono::{DateTime, Local};
use iced::{
    Color, Element, Event, Length, Point, Rectangle, Subscription, Task, Theme,
    advanced::{mouse, subscription},
    alignment::{Horizontal, Vertical},
    event,
    padding::{left, top},
    time::{self, Duration},
    widget::{Column, Container, Stack, Text, column, container, stack, text},
    window::Id,
};
use iced_layershell::{
    reexport::{Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings},
    to_layer_message,
};
use std::{
    collections::HashMap,
    process::Command,
    sync::{Arc, Mutex},
    thread,
};
use zbus::Connection;

use crate::{
    config::{BAR_NAMESPACE, BAR_WIDTH, GAPS, TOOLTIP_RETRIES},
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

#[derive(Debug, Clone)]
pub enum MouseEvent {
    Workspace(u64),
    Window(u64),
    MprisPlayer(String),
    Tooltip(container::Id),
}

#[to_layer_message(multi)]
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

    MprisEvent(MprisEvent),
    PlayPause(String),
    NextSong(String),
    StopPlayer(String),
    ChangeVolume(i32),

    CreateTooltipCanvas,
    TooltipMeasured(container::Id, Option<Rectangle>),

    ErrorMessage(String),
    NoOp,
}

pub struct Bar {
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

        (
            Self {
                tooltips,
                tooltip_canvas: Id::unique(),
                time_module,
                battery_module,
                niri_module: NiriModule::new(icon_cache.clone()),
                mpris_module: MprisModule::new(),
                cava_module: CavaModule::new(),
            },
            Task::batch(vec![
                // create_client(),
                Task::done(Message::CreateTooltipCanvas),
            ]),
        )
    }

    pub fn remove_id(&mut self, _id: Id) {}

    pub fn namespace(&self) -> String {
        String::from(BAR_NAMESPACE)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions: Vec<Subscription<Message>> = Vec::with_capacity(8);

        subscriptions.push(event::listen().map(Message::IcedEvent));

        subscriptions.push(
            subscription::from_recipe(NiriSubscriptionRecipe)
                .map(Message::NiriOutput),
        );
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

        subscriptions.push(
            time::every(Duration::from_secs(1))
                .map(|_| Message::Tick(Local::now())),
        );

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
                self.time_module.time = time;
                self.battery_module.fetch_battery_info();
                Task::none()
            }
            Message::NiriOutput(output) => {
                self.niri_module.handle_niri_output(output)
            }
            Message::NiriAction(action) => self.niri_module.handle_action(action),
            Message::MouseEntered(event) => {
                match event {
                    MouseEvent::MprisPlayer(name) => 'block: {
                        let Some(player) = self.mpris_module.players.get(&name)
                        else {
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
                                move |rect| {
                                    Message::TooltipMeasured(id.clone(), rect)
                                },
                            );
                        }
                    }
                    MouseEvent::Window(w_id) => 'block: {
                        let Some(window) = self.niri_module.windows.get(&w_id)
                        else {
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
                                move |rect| {
                                    Message::TooltipMeasured(id.clone(), rect)
                                },
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
                                move |rect| {
                                    Message::TooltipMeasured(id.clone(), rect)
                                },
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
                                return container::visible_bounds(id.clone())
                                    .map(move |rect| {
                                        Message::TooltipMeasured(id.clone(), rect)
                                    });
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
            Message::MprisEvent(event) => self.mpris_module.on_event(event),
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
                thread::spawn(move || {
                    let _ = Command::new("wpctl")
                        .args([
                            "set-volume",
                            "@DEFAULT_SINK@",
                            &format!("{value}%{sign}"),
                        ])
                        .output();
                });

                Task::none()
            }
            Message::NoOp => Task::none(),
            _ => unreachable!(),
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
            .width(Length::Fixed(BAR_WIDTH as f32 - GAPS as f32 * 2.0))
            .height(Length::Fill)
            .style(rounded_corners);

        Container::new(bar)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(GAPS)
            .into()
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

                    let position =
                        tooltip.position.unwrap_or(Point::new(0.0, 0.0));

                    const TOOLTIP_PADDING: u16 = 7;
                    const TEXT_SIZE: u16 = 16;

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
                        - content.lines().count() as f32
                            * (TEXT_SIZE as f32 / 2.0)
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
        .padding(left(BAR_WIDTH as f32 - GAPS as f32 + 0.04))
        .into()
    }

    pub fn view(&self, id: Id) -> Element<Message> {
        if id == self.tooltip_canvas {
            return self.view_canvas();
        } else {
            self.view_bar()
        }
    }

    pub fn style(&self, theme: &Theme) -> iced_layershell::Appearance {
        iced_layershell::Appearance {
            background_color: Color::TRANSPARENT,
            text_color: theme.palette().text,
        }
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }
}
