use iced::{
    Element, Length,
    widget::{Container, Text, container, text},
};

use crate::{
    Message, config::Config, services::time::TimeService, style::styled_tooltip,
};

pub struct TimeView {
    pub id: container::Id,
}

impl<'a> TimeView {
    pub fn view(&self, service: &TimeService, _config: &Config) -> Element<'a, Message> {
        let time = service.time.format("%I\n%M").to_string();
        let tooltip = Text::new(service.time.format("%a %b %-d\n%-m/%-d/%y").to_string());
        let content = Container::new(text(time).size(16))
            .center_x(Length::Fill)
            .id(self.id.clone());

        styled_tooltip(content, tooltip)
    }
}

impl TimeView {
    pub fn new() -> Self {
        Self {
            id: container::Id::unique(),
        }
    }
}
