use std::any::Any;

use iced::{
    Length,
    mouse::{Interaction, ScrollDelta},
    widget::{
        self, Column, Container, Image, MouseArea, Row, Text, text::Shaping
    },
};
use rustc_hash::FxHashMap;

use crate::{
    Element, Message, config,
    modules::{BarPosition, Modules, ViewTrait, mpris::service::MprisPlayer},
    utils::style::container_style,
};

pub struct MprisView {
    config: config::Mpris,
    pub position: BarPosition,
    player_views: FxHashMap<String, MprisPlayerView>,
}

#[profiling::all_functions]
impl ViewTrait<Modules> for MprisView {
    fn view<'a>(
        &'a self,
        modules: &'a Modules,
        layout: &'a config::Layout,
    ) -> Element<'a> {
        let service = modules.mpris.as_ref().expect("mpris should not be None");
        if layout.anchor.vertical() {
            service
                .players
                .iter()
                .fold(
                    Column::new().spacing(5).padding(5),
                    |col, (name, player)| {
                        if let Some(player_view) = self.player_views.get(name) {
                            col.push(player_view.view(
                                player,
                                &self.config,
                                layout,
                            ))
                        } else {
                            col
                        }
                    },
                )
                .into()
        } else {
            service
                .players
                .iter()
                .fold(
                    Row::new().spacing(5).padding(5),
                    |row, (name, player)| {
                        if let Some(player_view) = self.player_views.get(name) {
                            row.push(player_view.view(
                                player,
                                &self.config,
                                layout,
                            ))
                        } else {
                            row
                        }
                    },
                )
                .into()
        }
    }

    fn position(&self) -> BarPosition {
        self.position
    }

    fn tooltip<'a>(
        &'a self,
        modules: &'a Modules,
        id: &widget::Id,
    ) -> Option<Element<'a>> {
        let service = modules.mpris.as_ref().expect("mpris should not be None");
        self.player_views.iter().find_map(|(player_name, view)| {
            if view.id == *id {
                service
                    .players
                    .iter()
                    .find(|(name, _)| name == player_name)
                    .and_then(|(_, player)| view.render_tooltip(player))
            } else {
                None
            }
        })
    }

    fn synchronize(&mut self, modules: &Modules) {
        let service = modules.mpris.as_ref().expect("mpris should not be None");
        let player_names: Vec<&String> = service
            .players
            .iter()
            .map(|(player_name, _)| player_name)
            .collect();
        self.player_views
            .retain(|name, _| player_names.contains(&name));

        for name in player_names {
            self.player_views
                .entry(name.clone())
                .or_insert_with(MprisPlayerView::new);
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl MprisView {
    pub fn new(config: config::Mpris, position: BarPosition) -> Self {
        Self {
            config,
            position,
            player_views: FxHashMap::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct MprisPlayerView {
    pub id: widget::Id,
}

impl MprisPlayerView {
    fn new() -> Self {
        Self {
            id: widget::Id::unique(),
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
    ) -> Element<'a> {
        let content: Element<'a> = if let Some(art) = &player.art {
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
                container_style(container, &config.placeholder_style, layout);
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

        let content = Container::new(
            mouse_area
                .interaction(Interaction::Pointer)
                .on_enter(Message::OpenTooltip(self.id.clone()))
                .on_exit(Message::CloseTooltip(self.id.clone())),
        )
        .id(self.id.clone());

        content.into()
    }

    pub fn render_tooltip(
        &'a self,
        player: &'a MprisPlayer,
    ) -> Option<Element<'a>> {
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
