use chrono::{DateTime, Local};
use iced::{
    Alignment, Background, Border, Color, Element, Event, Font, Length, Subscription, Task, Theme,
    advanced::mouse,
    border::{rounded, top_right},
    event,
    font::{Family, Weight},
    padding::top,
    time::{self, Duration},
    widget::{column, container, text},
    window::Id,
};
use iced_layershell::{
    actions::IcedNewPopupSettings,
    build_pattern::{MainSettings, daemon},
    reexport::{Anchor, KeyboardInteractivity, NewLayerShellSettings},
    settings::{LayerShellSettings, StartMode},
    to_layer_message,
};
use std::collections::HashMap;

use iced_runtime::{Action, task, window::Action as WindowAction};
use tokio::time::Instant;

use crate::{
    battery_widget::{BatteryInfo, battery_icon, fetch_battery_info},
    utils::align_clock,
};

extern crate starship_battery as battery;

mod battery_widget;
mod utils;

const BAR_WIDTH: u32 = 45;
const GAPS: i32 = 3;
const ANIMATION_DURATION: Duration = Duration::from_millis(200);

const FIRA_CODE_BYTES: &[u8] = include_bytes!("../fonts/FiraCodeNerdFontMono-Medium.ttf");
const FIRA_CODE: Font = Font {
    family: Family::Name("FiraCode Nerd Font Mono"),
    weight: Weight::Medium,
    ..Font::DEFAULT
};

#[derive(Clone, Copy)]
struct Tooltip<State: TooltipMarkerState> {
    id: Id,
    state: State,
}

#[derive(Clone)]
struct Hidden;
#[derive(Clone)]
struct AnimatingIn {
    content: String,
    start: Instant,
}
#[derive(Clone)]
struct AnimatingOut {
    content: String,
    start: Instant,
}
#[derive(Clone)]
struct Visible {
    content: String,
}

impl Tooltip<Hidden> {
    fn animate_in(self, content: String) -> Tooltip<AnimatingIn> {
        Tooltip {
            id: self.id,
            state: AnimatingIn {
                content,
                start: Instant::now(),
            },
        }
    }
}

impl Tooltip<AnimatingIn> {
    fn to_visible(self) -> Tooltip<Visible> {
        Tooltip {
            id: self.id,
            state: Visible {
                content: self.state.content,
            },
        }
    }

    fn animate_out(self) -> Tooltip<AnimatingOut> {
        Tooltip {
            id: self.id,
            state: AnimatingOut {
                content: self.state.content,
                start: Instant::now(),
            },
        }
    }
}

impl Tooltip<Visible> {
    fn animate_out(self) -> Tooltip<AnimatingOut> {
        Tooltip {
            id: self.id,
            state: AnimatingOut {
                content: self.state.content,
                start: Instant::now(),
            },
        }
    }
}

impl Tooltip<AnimatingOut> {
    fn animate_in(self) -> Tooltip<AnimatingIn> {
        Tooltip {
            id: self.id,
            state: AnimatingIn {
                content: self.state.content,
                start: Instant::now(),
            },
        }
    }

    fn to_hidden(self) -> Tooltip<Hidden> {
        Tooltip {
            id: self.id,
            state: Hidden {},
        }
    }
}

trait TooltipMarkerState {}
impl TooltipMarkerState for Hidden {}
impl TooltipMarkerState for AnimatingIn {}
impl TooltipMarkerState for AnimatingOut {}
impl TooltipMarkerState for Visible {}

enum TooltipState {
    Hidden(Tooltip<Hidden>),
    AnimatingIn(Tooltip<AnimatingIn>),
    Visible(Tooltip<Visible>),
    AnimatingOut(Tooltip<AnimatingOut>),
}

impl TooltipState {
    fn id(&self) -> Id {
        match self {
            TooltipState::Hidden(t) => t.id,
            TooltipState::AnimatingIn(t) => t.id,
            TooltipState::Visible(t) => t.id,
            TooltipState::AnimatingOut(t) => t.id,
        }
    }
}

#[tokio::main]
pub async fn main() -> Result<(), iced_layershell::Error> {
    daemon(Bar::namespace, Bar::update, Bar::view, Bar::remove_id)
        .subscription(Bar::subscription)
        .style(Bar::style)
        .theme(Bar::theme)
        .settings(MainSettings {
            fonts: vec![FIRA_CODE_BYTES.into()],
            default_font: FIRA_CODE,
            layer_settings: LayerShellSettings {
                size: Some((BAR_WIDTH, 0)),
                exclusive_zone: BAR_WIDTH as i32 - GAPS,
                anchor: Anchor::Left | Anchor::Top | Anchor::Bottom,
                margin: (GAPS, 0, GAPS, GAPS),
                keyboard_interactivity: KeyboardInteractivity::None,
                start_mode: StartMode::Active,
                ..Default::default()
            },
            ..Default::default()
        })
        .run_with(|| Bar::new())
}

fn rounded_corners(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.8))),
        border: rounded(12),
        ..Default::default()
    }
}

fn tooltip_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.8))),
        border: Border {
            radius: top_right(12).bottom_right(12),
            ..Default::default()
        },
        ..Default::default()
    }
}

struct Bar {
    time: DateTime<Local>,
    clock_aligned: bool,
    battery_info: Option<Vec<BatteryInfo>>,
    tooltip_state: TooltipState,
    ids: HashMap<Id, WindowType>,
}

#[derive(Clone)]
enum WindowType {
    BatteryTooltip,
}

#[to_layer_message(multi)]
#[derive(Debug, Clone)]
enum Message {
    IcedEvent(Event),
    Tick(DateTime<Local>),
    AlignClock,
    BatteryUpdate(Vec<BatteryInfo>),
    ErrorMessage(String),
    AnimationTick(Instant),
    MouseEntered,
    MouseExited,
}

impl Bar {
    fn new() -> (Self, Task<Message>) {
        let battery_info = match fetch_battery_info() {
            Message::BatteryUpdate(info) => Some(info),
            _ => unreachable!(),
        };
        (
            Self {
                clock_aligned: false,
                time: Local::now(),
                battery_info,
                tooltip_state: TooltipState::Hidden(Tooltip {
                    id: Id::unique(),
                    state: Hidden,
                }),
                ids: HashMap::new(),
            },
            align_clock(),
        )
    }

    fn remove_id(&mut self, id: Id) {
        self.ids.remove(&id);
    }

    fn namespace(&self) -> String {
        String::from("Iced Bar")
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions: Vec<Subscription<Message>> = Vec::with_capacity(3);

        subscriptions.push(event::listen().map(Message::IcedEvent));
        subscriptions.push(time::every(Duration::from_secs(1)).map(|_| fetch_battery_info()));
        if self.clock_aligned {
            subscriptions
                .push(time::every(Duration::from_secs(60)).map(|_| Message::Tick(Local::now())));
        }
        if matches!(
            self.tooltip_state,
            TooltipState::AnimatingIn { .. } | TooltipState::AnimatingOut { .. }
        ) {
            subscriptions.push(
                time::every(Duration::from_millis(1000 / 60))
                    .map(|_| Message::AnimationTick(Instant::now())),
            )
        }
        Subscription::batch(subscriptions)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::IcedEvent(event) => {
                if let Event::Mouse(mouse::Event::CursorLeft) = event {
                    return Task::done(Message::MouseExited);
                }
                // println!("{event:?}");
                Task::none()
            }
            Message::Tick(time) => {
                self.time = time;
                Task::none()
            }
            Message::AlignClock => {
                self.clock_aligned = true;
                self.time = Local::now();
                Task::none()
            }
            Message::BatteryUpdate(info) => {
                self.battery_info = Some(info);
                Task::none()
            }
            Message::MouseEntered => {
                let tooltip_content: String = if let Some(info) = &self.battery_info {
                    info.iter()
                        .enumerate()
                        .map(|(i, bat)| {
                            format!(
                                "Battery {}: {}% ({})",
                                i + 1,
                                bat.percentage * 100.0,
                                bat.state
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    "No Battery Info".to_string()
                };

                let task = Task::done(Message::NewPopUp {
                    settings: IcedNewPopupSettings {
                        size: (100, 50),
                        position: (BAR_WIDTH as i32 - GAPS as i32 - 1, 0),
                    },
                    id: self.tooltip_state.id(),
                });

                match &self.tooltip_state {
                    TooltipState::Hidden(tooltip) => {
                        self.tooltip_state =
                            TooltipState::AnimatingIn(tooltip.clone().animate_in(tooltip_content));
                        return task;
                    }
                    TooltipState::AnimatingOut(tooltip) => {
                        self.tooltip_state =
                            TooltipState::AnimatingIn(tooltip.clone().animate_in());
                    }
                    _ => {}
                };

                Task::none()
            }
            Message::MouseExited => {
                match &self.tooltip_state {
                    TooltipState::Visible(tooltip) => {
                        self.tooltip_state =
                            TooltipState::AnimatingOut(tooltip.clone().animate_out());
                    }
                    TooltipState::AnimatingIn(tooltip) => {
                        self.tooltip_state =
                            TooltipState::AnimatingOut(tooltip.clone().animate_out());
                    }
                    _ => {}
                };

                Task::none()
            }
            Message::AnimationTick(now) => {
                match &self.tooltip_state {
                    TooltipState::AnimatingIn(tooltip) => {
                        if now.duration_since(tooltip.state.start) >= ANIMATION_DURATION {
                            self.tooltip_state =
                                TooltipState::Visible(tooltip.clone().to_visible());
                        }
                    }
                    TooltipState::AnimatingOut(tooltip) => {
                        if now.duration_since(tooltip.state.start) >= ANIMATION_DURATION {
                            let task = task::effect(Action::Window(WindowAction::Close(
                                tooltip.clone().id,
                            )));
                            self.tooltip_state = TooltipState::Hidden(tooltip.clone().to_hidden());
                            return task;
                        }
                    }
                    TooltipState::Hidden(_tooltip) => {}
                    TooltipState::Visible(_tooltip) => {}
                }
                Task::none()
            }
            Message::ErrorMessage(msg) => {
                eprintln!("{}", msg);
                Task::none()
            }
            _ => unreachable!(),
        }
    }

    fn view(&self, id: Id) -> Element<Message> {
        if id == self.tooltip_state.id() {
            container(column![text("󱄅").size(32)])
                .width(Length::Fill)
                .height(Length::Fill)
                .style(tooltip_style)
                .into()
        } else {
            let time: String = self.time.format("%H\n%M").to_string();

            let center = column![
                text("󱄅").size(32),
                battery_icon(self.battery_info.as_ref()),
                text(time).size(16)
            ]
            .align_x(Alignment::Center)
            .width(Length::Fill)
            .height(Length::Fill);

            container(column![center].width(Length::Fill))
                .width(Length::Fixed(BAR_WIDTH as f32 - GAPS as f32))
                .height(Length::Fill)
                .padding(top(GAPS as f32).bottom(GAPS as f32))
                .style(rounded_corners)
                .into()
        }
    }

    fn style(&self, theme: &Theme) -> iced_layershell::Appearance {
        use iced_layershell::Appearance;
        Appearance {
            background_color: Color::TRANSPARENT,
            // Color::from_rgba(0.0, 0.0, 0.0, 0.8),
            text_color: theme.palette().text,
        }
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }
}
