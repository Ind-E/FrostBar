use bimap::BiMap;
use chrono::{DateTime, Local};
use iced::{
    Background, Border, Color, Element, Event, Font, Length, Rectangle, Subscription, Task, Theme,
    advanced::{mouse, subscription},
    alignment::{Horizontal, Vertical},
    border::{rounded, top_right},
    event,
    font::{Family, Weight},
    padding::top,
    time::{self, Duration},
    widget::{
        Column, Container, Text, column,
        container::{self, StyleFn},
        stack, text,
    },
    window::Id,
};
use iced_layershell::{
    actions::IcedNewPopupSettings,
    build_pattern::{MainSettings, daemon},
    reexport::{Anchor, KeyboardInteractivity},
    settings::{LayerShellSettings, StartMode},
    to_layer_message,
};
use itertools::Itertools;
use niri_ipc::Request;
use std::sync::Arc;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use iced_runtime::{Action, task, window::Action as WindowAction};
use tokio::{
    sync::{
        Mutex,
        mpsc::{self, Sender},
    },
    time::Instant,
};

use crate::{
    battery_widget::{BatteryInfo, battery_icon, fetch_battery_info},
    niri::{IpcError, NiriEvents, NiriState, Window, Workspace, run_niri_request_handler},
    tooltip::{Hidden, Tooltip, TooltipState},
    utils::align_clock,
};

extern crate starship_battery as battery;

mod battery_widget;
mod niri;
mod tooltip;
mod utils;

const BAR_WIDTH: u32 = 45;
const GAPS: i32 = 3;
const ANIMATION_DURATION: Duration = Duration::from_millis(175);

const FIRA_CODE_BYTES: &[u8] = include_bytes!("../fonts/FiraCodeNerdFontMono-Medium.ttf");
const FIRA_CODE: Font = Font {
    family: Family::Name("FiraCode Nerd Font Mono"),
    weight: Weight::Medium,
    ..Font::DEFAULT
};

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

fn tooltip_style<'a>(opacity: f32) -> StyleFn<'a, Theme> {
    Box::new(move |_| container::Style {
        background: Some(Background::Color(Color::from_rgba(
            0.0,
            0.0,
            0.0,
            opacity * 0.8,
        ))),
        border: Border {
            radius: top_right(12).bottom_right(12),
            width: 0.928,
            ..Default::default()
        },
        ..Default::default()
    })
}

struct Bar {
    time: DateTime<Local>,
    clock_aligned: bool,
    battery_info: Option<Vec<BatteryInfo>>,
    tooltip_state: HashMap<container::Id, TooltipState>,
    ids: BiMap<container::Id, ItemsWithTooltips>,
    niri_state: NiriState,
    niri_request_sender: Sender<Request>,
}

#[derive(Debug, EnumIter, Hash, Eq, PartialEq, Clone)]
enum ItemsWithTooltips {
    Battery,
}

#[derive(Debug, Clone)]
enum MouseEnterEvent {
    Workspace(u8),
    Tooltip(ItemsWithTooltips),
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
    MouseEntered(MouseEnterEvent),
    MouseExited(MouseEnterEvent),
    MouseExitedBar,
    TooltipMeasured(Option<Rectangle>),
    NiriEvent(Result<niri_ipc::Event, IpcError>),
    WorkspaceClicked(u8),
    NoOp,
}

impl Bar {
    fn new() -> (Self, Task<Message>) {
        let battery_info = match fetch_battery_info() {
            Message::BatteryUpdate(info) => Some(info),
            _ => unreachable!(),
        };

        let mut ids = BiMap::new();

        for item in ItemsWithTooltips::iter() {
            ids.insert(container::Id::unique(), item);
        }

        let (request_tx, request_rx) = mpsc::channel(32);
        let request_socket = match niri_ipc::socket::Socket::connect() {
            Ok(sock) => Arc::new(Mutex::new(sock)),
            Err(e) => panic!("Failed to create niri request socket: {}", e),
        };

        tokio::spawn(run_niri_request_handler(request_rx, request_socket));
        (
            Self {
                clock_aligned: false,
                time: Local::now(),
                battery_info,
                tooltip_state: TooltipState::Hidden(Tooltip {
                    id: Id::unique(),
                    state: Hidden,
                }),
                ids,
                niri_state: NiriState::default(),
                niri_request_sender: request_tx,
            },
            align_clock(),
        )
    }

    fn remove_id(&mut self, _id: Id) {}

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
        subscriptions.push(subscription::from_recipe(NiriEvents).map(Message::NiriEvent));
        Subscription::batch(subscriptions)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::IcedEvent(event) => {
                if let Event::Mouse(mouse::Event::CursorLeft) = event {
                    return Task::done(Message::MouseExitedBar);
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
            Message::NiriEvent(event) => {
                println!("{:?}", event);
                if let Ok(event) = event {
                    self.niri_state.on_event(event);
                }

                Task::none()
            }
            Message::WorkspaceClicked(idx) => {
                let sender = self.niri_request_sender.clone();
                let request = niri_ipc::Request::Action(niri_ipc::Action::FocusWorkspace {
                    reference: niri_ipc::WorkspaceReferenceArg::Index(idx),
                });
                Task::perform(async move { sender.send(request).await.ok() }, |_| {
                    Message::NoOp
                })
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

                let task = container::visible_bounds(
                    self.ids
                        .get_by_right(&ItemsWithTooltips::Battery)
                        .unwrap()
                        .clone(),
                )
                .map(Message::TooltipMeasured);

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
            Message::MouseExitedBar => {
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
            Message::TooltipMeasured(rect) => {
                if let Some(rect) = rect {
                    Task::done(Message::NewPopUp {
                        settings: IcedNewPopupSettings {
                            size: (400, 300),
                            position: (
                                BAR_WIDTH as i32 - GAPS as i32 - 1,
                                (rect.y - rect.width / 4.0) as i32,
                            ),
                        },
                        id: self.tooltip_state.id(),
                    })
                } else {
                    Task::none()
                }
            }
            Message::ErrorMessage(msg) => {
                eprintln!("{}", msg);
                Task::none()
            }
            Message::NoOp => Task::none(),
            _ => unreachable!(),
        }
    }

    fn view(&self, id: Id) -> Element<Message> {
        if id == self.tooltip_state.id() {
            let progress = self.tooltip_state.progress();
            Container::new(
                self.tooltip_state
                    .content()
                    .lines()
                    .map(|line| {
                        Container::new(
                            Text::new(line)
                                .size(16)
                                // .color(Color::from_rgba(1.0, 1.0, 1.0, progress))
                                .line_height(text::LineHeight::Relative(1.0))
                                .shaping(text::Shaping::Basic)
                                .wrapping(text::Wrapping::None),
                        )
                        .width(Length::Fill)
                        .height(Length::Fixed(16.0))
                        .clip(true)
                    })
                    .fold(Column::new(), |col, text| col.push(text)),
            )
            .style(tooltip_style(1.0))
            .padding(7)
            .width(progress * 250.0)
            .clip(true)
            .into()
        } else {
            let time: String = self.time.format("%H\n%M").to_string();

            let top_section = Container::new(
                column![
                    text("󱄅").size(32),
                    battery_icon(
                        self.battery_info.as_ref(),
                        self.ids
                            .get_by_right(&ItemsWithTooltips::Battery)
                            .unwrap()
                            .clone()
                    ),
                    text(time).size(16)
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
                .map(|(_, ws)| Workspace {
                    output: &ws.output,
                    idx: &ws.idx,
                    is_active: &ws.is_active,
                    windows: self
                        .niri_state
                        .windows
                        .iter()
                        .filter_map(|(_, w)| {
                            (w.workspace_id == Some(ws.id)).then(|| Window {
                                title: &w.title,
                                id: &w.id,
                                icon: None,
                            })
                        })
                        .collect::<Vec<_>>(),
                })
                .fold(Column::new(), |col, ws| col.push(ws.to_widget()))
                .align_x(Horizontal::Center)
                .spacing(12);

            let middle_section = Container::new(ws)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Horizontal::Center)
                .align_y(Vertical::Center);

            let bottom_section = Container::new(
                column![
                    text("󱄅").size(32),
                    text("󱄅").size(32),
                    text("󱄅").size(32),
                    text("󱄅").size(32),
                ]
                .align_x(Horizontal::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Center)
            .align_y(Vertical::Bottom);

            let layout = stack![top_section, middle_section, bottom_section];

            Container::new(layout)
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
