use chrono::{DateTime, Local};
use iced::{
    Element, Length,
    widget::{Container, Text, container, text},
};

use crate::{Message, style::styled_tooltip};

pub struct TimeModule {
    pub time: DateTime<Local>,
    pub id: container::Id,
}

impl TimeModule {
    pub fn new() -> Self {
        Self {
            time: Local::now(),
            id: container::Id::unique(),
        }
    }

    pub fn to_widget<'a>(&self) -> Element<'a, Message> {
        let time = self.time.format("%I\n%M").to_string();
        let tooltip = Text::new(self.time.format("%a %b %-d\n%-m/%-d/%y").to_string());
        let content = Container::new(text(time).size(16))
            .center_x(Length::Fill)
            .id(self.id.clone());

        styled_tooltip(content, tooltip)
    }
}
