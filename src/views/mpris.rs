use iced::{
    Border, Color, Element, Length,
    border::Radius,
    mouse::Interaction,
    widget::{Column, Container, Image, MouseArea, Text, container, text::Shaping},
};

use crate::{
    Message,
    config::Config,
    services::mpris::{MprisPlayer, MprisService},
    style::styled_tooltip,
};

pub struct MprisView {}

impl<'a> MprisView {
    pub fn view(
        &'a self,
        service: &MprisService,
        config: &'a Config,
    ) -> Element<'a, Message> {
        service
            .players
            .values()
            .fold(Column::new().spacing(5).padding(5), |col, player| {
                col.push(MprisPlayerView::new().view(&player, &config))
            })
            .into()
    }
}

impl MprisView {
    pub fn new() -> Self {
        Self {}
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
    fn view(&self, player: &MprisPlayer, config: &Config) -> Element<'a, Message> {
        let content: Element<'a, Message> = if let Some(art) = &player.art {
            Container::new(Image::new(art)).into()
        } else {
            let layout = &config.layout;
            Container::new(
                Text::new("󰝚")
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
            Text::new(format!("{} - {}", artists, title)).shaping(Shaping::Advanced);

        let content = Container::new(
            MouseArea::new(content)
                .on_release(Message::PlayPause(player.name.clone()))
                .on_right_release(Message::NextSong(player.name.clone()))
                .on_middle_release(Message::StopPlayer(player.name.clone()))
                .interaction(Interaction::Pointer),
        )
        .id(self.id.clone());

        styled_tooltip(content, tooltip)
    }
}
