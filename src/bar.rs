use chrono::{DateTime, Local};
use iced::{
    Color, Element, Event, Length, Point, Rectangle, Subscription, Task, Theme,
    advanced::{mouse, subscription},
    alignment::{Horizontal, Vertical},
    event,
    mouse::ScrollDelta,
    padding::{left, top},
    time::{self, Duration},
    widget::{
        Column, Container, MouseArea, Scrollable, Stack, Text, canvas, column,
        container, stack, text,
    },
    window::Id,
};
use iced_layershell::{
    reexport::{Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings},
    to_layer_message,
};
use itertools::Itertools;
use std::collections::HashMap;
use zbus::Connection;

use tokio::process::Command as TokioCommand;

use crate::{
    config::{BAR_NAMESPACE, BAR_WIDTH, GAPS, TOOLTIP_RETRIES, VOLUME_PERCENT},
    dbus_proxy::PlayerProxy,
    icon_cache::IconCache,
    modules::{
        battery::BatteryState,
        cava::{CavaError, CavaEvents, CavaVisualizer, write_temp_cava_config},
        mpris::{MprisEvent, MprisListener, MprisState},
        niri::{NiriState, NiriSubscriptionRecipe},
    },
    style::{no_rail, rounded_corners, tooltip_style},
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
    TooltipMeasured(container::Id, Option<Rectangle>),
    NiriIpcEvent(niri_ipc::Event),
    NiriAction(niri_ipc::Action),
    CavaUpdate(Result<String, CavaError>),
    MprisEvent(MprisEvent),
    PlayPause(String),
    NextSong(String),
    StopPlayer(String),
    ChangeVolume(i32),
    CreateTooltipCanvas,
    ErrorMessage(String),
    NoOp,
}

pub struct Bar {
    time: DateTime<Local>,
    battery_module: BatteryState,
    tooltips: HashMap<container::Id, Tooltip>,
    tooltip_canvas: Id,

    window_tooltips: HashMap<u64, container::Id>,
    niri_state: NiriState,

    cava_visualizer: CavaVisualizer,

    // pub icon_cache: Arc<Mutex<IconCache>>,
    mpris_module: MprisState,
}

impl Bar {
    pub fn new() -> (Self, Task<Message>) {
        let battery_module = BatteryState::new();

        let mut tooltips = HashMap::new();
        tooltips.insert(battery_module.id.clone(), Tooltip::default());

        (
            Self {
                time: Local::now(),
                battery_module,
                tooltips,
                window_tooltips: HashMap::new(),
                niri_state: NiriState::new(IconCache::new()),
                cava_visualizer: CavaVisualizer {
                    bars: vec![0; 10],
                    cache: iced::widget::canvas::Cache::new(),
                },
                // icon_cache: Arc::new(Mutex::new(IconCache::new())),
                mpris_module: MprisState::new(),
                tooltip_canvas: Id::unique(),
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
                .map(Message::NiriIpcEvent),
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
                self.time = time;
                self.battery_module.fetch_battery_info();
                Task::none()
            }
            Message::NiriIpcEvent(event) => {
                self.niri_state.handle_ipc_event(event)
            }
            Message::NiriAction(action) => self.niri_state.handle_action(action),
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
                    MouseEvent::Window(w_id) => {
                        let id = self
                            .window_tooltips
                            .entry(w_id)
                            .or_insert_with(container::Id::unique)
                            .clone();
                        let tooltip = self
                            .tooltips
                            .entry(id.clone())
                            .or_insert_with(|| Tooltip::default());

                        if tooltip.state == TooltipState::Hidden
                            && let Some(window) = self
                                .niri_state
                                .windows
                                .values()
                                .find(|w| w.id == w_id)
                        {
                            tooltip.content = window.title.clone();
                            tooltip.state = TooltipState::Measuring(0);

                            return container::visible_bounds(id.clone()).map(
                                move |rect| {
                                    Message::TooltipMeasured(id.clone(), rect)
                                },
                            );
                        }
                    }
                    MouseEvent::Workspace(id) => {
                        self.niri_state.hovered_workspace_id = Some(id);
                    }
                    MouseEvent::Tooltip(id) => {
                        if let Some(tooltip) = self.tooltips.get_mut(&id)
                            && tooltip.state == TooltipState::Hidden
                        {
                            tooltip.content = Some(self.battery_module.tooltip());
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
                self.niri_state.hovered_workspace_id = None;
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
                    MouseEvent::Window(id) => {
                        if let Some(id) = self.window_tooltips.get(&id)
                            && let Some(tooltip) = self.tooltips.get_mut(id)
                        {
                            tooltip.state = TooltipState::Hidden;
                        }
                    }
                    MouseEvent::Workspace(..) => {
                        self.niri_state.hovered_workspace_id = None;
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
                eprintln!("error message: {}", msg);
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
                        eprintln!("cava error: {}", e);
                    }
                };
                Task::none()
            }
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
                Task::perform(
                    async move {
                        let _ = TokioCommand::new("wpctl")
                            .args(&[
                                "set-volume",
                                "@DEFAULT_SINK@",
                                &format!("{value}%{sign}"),
                            ])
                            .output()
                            .await;
                    },
                    |_| Message::NoOp,
                )
            }
            Message::NoOp => Task::none(),
            _ => unreachable!(),
        }
    }

    fn view_bar(&self) -> Element<Message> {
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

        let top_section = Container::new(
            column![
                text("󱄅").size(28),
                self.battery_module.to_widget(),
                text(time).size(16),
                cava_visualizer,
                self.mpris_module.to_widget(),
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
            .fold(Column::new(), |col, (_, ws)| {
                col.push(
                    ws.to_widget(
                        self.niri_state
                            .hovered_workspace_id
                            .is_some_and(|id| id == ws.id),
                        &self.window_tooltips,
                    ),
                )
            })
            .align_x(Horizontal::Center)
            .spacing(10);

        let middle_section = Container::new(
            Scrollable::new(Container::new(ws).align_y(Vertical::Center))
                .height(570)
                .style(no_rail),
        )
        .center_y(Length::Fill);

        let tray_items = column![];

        let bottom_section = Container::new(tray_items)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Center)
            .align_y(Vertical::Bottom);

        let layout = stack![top_section, middle_section, bottom_section];

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
                                    .line_height(1.0),
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
