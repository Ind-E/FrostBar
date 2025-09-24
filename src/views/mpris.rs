use iced::{
    Border, Color, Element, Length,
    border::Radius,
    mouse::{Interaction, ScrollDelta},
    widget::{Column, Container, Image, MouseArea, Text, container, text::Shaping},
};

use crate::{
    Message,
    config::{self, MouseTrigger},
    services::mpris::{MprisPlayer, MprisService},
    style::styled_tooltip,
    views::BarPosition,
};

pub struct MprisView {
    config: config::Mpris,
    pub position: BarPosition,
}

impl<'a> MprisView {
    pub fn view(
        &'a self,
        service: &'a MprisService,
        layout: &'a config::Layout,
    ) -> Element<'a, Message> {
        service
            .players
            .values()
            .fold(Column::new().spacing(5).padding(5), |col, player| {
                col.push(MprisPlayerView::new().view(player, &self.config, layout))
            })
            .into()
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
            Container::new(
                Text::new(config.placeholder.clone())
                    .size(20)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center(),
            )
            .padding(5)
            .width(layout.width - layout.gaps as u32 * 4)
            .height(layout.width - layout.gaps as u32 * 4)
            .center_x(Length::Fill)
            .style(|_| container::Style {
                border: Border {
                    color: Color::WHITE,
                    width: 1.0,
                    radius: Radius::new(1),
                },
                ..Default::default()
            })
            .into()
        };

        let raw_artists = player.artists.clone().unwrap_or_else(|| "[]".to_string());
        let raw_title = player.title.clone().unwrap_or_else(|| "\"\"".to_string());

        let artists = raw_artists
            .trim_start_matches('[')
            .trim_end_matches(']')
            .split(',')
            .map(|s| s.trim().trim_matches('"'))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", ");

        let title = raw_title.trim().trim_matches('"');

        let tooltip =
            Text::new(format!("{artists} - {title}")).shaping(Shaping::Advanced);

        let mut mouse_area = MouseArea::new(content);

        let mut scroll_binds = (None, None);

        for bind in &config.binds {
            match bind.trigger {
                MouseTrigger::MouseLeft => {
                    mouse_area = mouse_area.on_release(Message::MediaControl(
                        bind.action,
                        player.name.clone(),
                    ));
                }

                MouseTrigger::MouseRight => {
                    mouse_area = mouse_area.on_right_release(Message::MediaControl(
                        bind.action,
                        player.name.clone(),
                    ));
                }

                MouseTrigger::MouseMiddle => {
                    mouse_area = mouse_area.on_middle_release(Message::MediaControl(
                        bind.action,
                        player.name.clone(),
                    ));
                }
                MouseTrigger::ScrollUp => {
                    scroll_binds.0 = Some(bind.action);
                }

                MouseTrigger::ScrollDown => {
                    scroll_binds.1 = Some(bind.action);
                }
            }
        }

        if scroll_binds.0.is_some() || scroll_binds.1.is_some() {
            mouse_area = mouse_area.on_scroll(move |delta| {
                let (x, y) = match delta {
                    ScrollDelta::Lines { x, y } | ScrollDelta::Pixels { x, y } => (x, y),
                };

                if (y > 0.0 || x < 0.0)
                    && let Some(scroll_up) = scroll_binds.0
                {
                    return Message::MediaControl(scroll_up, player.name.clone());
                } else if let Some(scroll_down) = scroll_binds.1 {
                    return Message::MediaControl(scroll_down, player.name.clone());
                }
                unreachable!()
            });
        }

        let content = Container::new(mouse_area.interaction(Interaction::Pointer))
            .id(self.id.clone());

        styled_tooltip(content, tooltip)
    }
}
