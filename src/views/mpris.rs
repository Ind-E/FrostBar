use iced::{
    Border, Color, Element, Length,
    border::Radius,
    mouse::{Interaction, ScrollDelta},
    widget::{
        Column, Container, Image, MouseArea, Row, Text, container,
        text::Shaping,
    },
};

use crate::{
    Message,
    config::{self},
    services::mpris::{MprisPlayer, MprisService},
    style::styled_tooltip,
    views::BarPosition,
};

pub struct MprisView {
    config: config::Mpris,
    pub position: BarPosition,
}

#[profiling::all_functions]
impl<'a> MprisView {
    pub fn view(
        &'a self,
        service: &'a MprisService,
        layout: &'a config::Layout,
    ) -> Element<'a, Message> {
        if layout.anchor.vertical() {
            service
                .players
                .values()
                .fold(Column::new().spacing(5).padding(5), |col, player| {
                    col.push(MprisPlayerView::new().view(
                        player,
                        &self.config,
                        layout,
                    ))
                })
                .into()
        } else {
            service
                .players
                .values()
                .fold(Row::new().spacing(5).padding(5), |col, player| {
                    col.push(MprisPlayerView::new().view(
                        player,
                        &self.config,
                        layout,
                    ))
                })
                .into()
        }
    }
}

impl MprisView {
    pub fn new(config: config::Mpris, position: BarPosition) -> Self {
        Self { config, position }
    }
}

#[derive(Clone, Debug)]
pub struct MprisPlayerView {
    pub id: container::Id,
}

impl MprisPlayerView {
    fn new() -> Self {
        Self {
            id: container::Id::unique(),
        }
    }
}

#[profiling::all_functions]
impl<'a> MprisPlayerView {
    fn view(
        &self,
        player: &'a MprisPlayer,
        config: &'a config::Mpris,
        layout: &config::Layout,
    ) -> Element<'a, Message> {
        let content: Element<'a, Message> = if let Some(art) = &player.art {
            Container::new(Image::new(art)).into()
        } else {
            let mut container = Container::new(
                Text::new(config.placeholder.clone())
                    .size(20)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center(),
            )
            .padding(5)
            .width(layout.width - layout.gaps as u32 * 4)
            .height(layout.width - layout.gaps as u32 * 4)
            .style(|_| container::Style {
                border: Border {
                    color: Color::WHITE,
                    width: 1.0,
                    radius: Radius::new(1),
                },
                ..Default::default()
            });
            if layout.anchor.vertical() {
                container = container.center_x(Length::Fill);
            } else {
                container = container.center_y(Length::Fill);
            }
            container.into()
        };

        let raw_artists =
            player.artists.clone().unwrap_or_else(|| "[]".to_string());
        let raw_title =
            player.title.clone().unwrap_or_else(|| "\"\"".to_string());

        let artists = raw_artists
            .trim_start_matches('[')
            .trim_end_matches(']')
            .split(',')
            .map(|s| s.trim().trim_matches('"'))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", ");

        let title = raw_title.trim().trim_matches('"');

        let tooltip = Text::new(format!("{artists} - {title}"))
            .shaping(Shaping::Advanced);

        let binds = &config.binds;

        if binds.mouse_left.is_some()
            || binds.mouse_right.is_some()
            || binds.mouse_middle.is_some()
            || binds.scroll_up.is_some()
            || binds.scroll_down.is_some()
        {
            let mut mouse_area = MouseArea::new(content);
            if let Some(left) = &binds.mouse_left {
                mouse_area = mouse_area.on_release(Message::MediaControl(
                    *left,
                    player.name.clone(),
                ));
            }

            if let Some(right) = &binds.mouse_right {
                mouse_area = mouse_area.on_right_release(
                    Message::MediaControl(*right, player.name.clone()),
                );
            }

            if let Some(middle) = &binds.mouse_middle {
                mouse_area = mouse_area.on_middle_release(
                    Message::MediaControl(*middle, player.name.clone()),
                );
            }

            if binds.scroll_up.is_some() || binds.scroll_down.is_some() {
                mouse_area = mouse_area.on_scroll(move |delta| {
                    let (x, y) = match delta {
                        ScrollDelta::Lines { x, y }
                        | ScrollDelta::Pixels { x, y } => (x, y),
                    };

                    if (y > 0.0 || x < 0.0)
                        && let Some(scroll_up) = &binds.scroll_up
                    {
                        return Message::MediaControl(
                            *scroll_up,
                            player.name.clone(),
                        );
                    } else if let Some(scroll_down) = &binds.scroll_down {
                        return Message::MediaControl(
                            *scroll_down,
                            player.name.clone(),
                        );
                    }
                    unreachable!()
                });
            }

            let content =
                Container::new(mouse_area.interaction(Interaction::Pointer))
                    .id(self.id.clone());

            return styled_tooltip(content, tooltip, &layout.anchor);
        }

        let content = Container::new(content).id(self.id.clone());

        styled_tooltip(content, tooltip, &layout.anchor)
    }
}
