use iced::{
    Element, Length,
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
    style::container_style,
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

    pub fn render_player_tooltip(
        &'a self,
        service: &'a MprisService,
        id: &container::Id,
    ) -> Option<Element<'a, Message>> {
        // service.players.values().find_map(|p| {
        //     if p.id == *id {
        //         p.render_tooltip()
        //     } else {
        //         None
        //     }
        // })
        None
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
        layout: &'a config::Layout,
    ) -> Element<'a, Message> {
        let content: Element<'a, Message> = if let Some(art) = &player.art {
            Container::new(Image::new(art)).into()
        } else {
            let container = Container::new(
                Text::new(config.placeholder.clone())
                    .size(20)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center(),
            );
            let container =
                container_style(container, &config.placeholder_style, &layout);
            if layout.anchor.vertical() {
                container
                    .center_x(Length::Fill)
                    .height(layout.width - 10)
                    .into()
            } else {
                container
                    .center_y(Length::Fill)
                    .width(layout.width - 10)
                    .into()
            }
        };

        let binds = &config.binds;

        let mut mouse_area = MouseArea::new(content)
            .on_enter(Message::OpenTooltip(self.id.clone()))
            .on_exit(Message::CloseTooltip(self.id.clone()));

        if let Some(left) = &binds.mouse_left {
            mouse_area = mouse_area
                .on_release(Message::MediaControl(*left, player.name.clone()));
        }

        if let Some(double) = &binds.double_click {
            mouse_area = mouse_area.on_double_click(Message::MediaControl(
                *double,
                player.name.clone(),
            ));
        }

        if let Some(right) = &binds.mouse_right {
            mouse_area = mouse_area.on_right_release(Message::MediaControl(
                *right,
                player.name.clone(),
            ));
        }

        if let Some(middle) = &binds.mouse_middle {
            mouse_area = mouse_area.on_middle_release(Message::MediaControl(
                *middle,
                player.name.clone(),
            ));
        }

        if binds.scroll_up.is_some()
            || binds.scroll_down.is_some()
            || binds.scroll_left.is_some()
            || binds.scroll_right.is_some()
        {
            mouse_area = mouse_area.on_scroll(move |delta| {
                let (x, y) = match delta {
                    ScrollDelta::Lines { x, y }
                    | ScrollDelta::Pixels { x, y } => (x, y),
                };

                if y > 0.0
                    && let Some(scroll_up) = &binds.scroll_up
                {
                    Message::MediaControl(*scroll_up, player.name.clone())
                } else if y < 0.0
                    && let Some(scroll_down) = &binds.scroll_down
                {
                    Message::MediaControl(*scroll_down, player.name.clone())
                } else if x < 0.0
                    && let Some(scroll_right) = &binds.scroll_right
                {
                    Message::MediaControl(*scroll_right, player.name.clone())
                } else if x > 0.0
                    && let Some(scroll_left) = &binds.scroll_left
                {
                    Message::MediaControl(*scroll_left, player.name.clone())
                } else {
                    Message::NoOp
                }
            });
        }

        let content =
            Container::new(mouse_area.interaction(Interaction::Pointer))
                .id(self.id.clone());

        content.into()
    }

    pub fn render_tooltip(
        &'a self,
        player: &'a MprisPlayer,
    ) -> Option<Element<'a, Message>> {
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

        Some(
            Text::new(format!("{artists} - {title}"))
                .shaping(Shaping::Advanced)
                .into(),
        )
    }
}
