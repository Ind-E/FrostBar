use iced::{
    Element, Length,
    widget::{Container, Text, container, text},
};

use crate::{
    Message, config, services::time::TimeService, style::styled_tooltip,
    views::BarPosition,
};

pub struct TimeView {
    pub id: container::Id,
    config: config::Time,
    pub position: BarPosition,
}

impl<'a> TimeView {
    pub fn view(&self, service: &TimeService) -> Element<'a, Message> {
        let time = service.time.format(&self.config.format).to_string();
        let tooltip =
            Text::new(service.time.format(&self.config.tooltip_format).to_string());
        let content = Container::new(text(time).size(16))
            .center_x(Length::Fill)
            .id(self.id.clone());

        styled_tooltip(content, tooltip)
    }
}

impl TimeView {
    pub fn new(config: config::Time, position: BarPosition) -> Self {
        Self {
            id: container::Id::unique(),
            config,
            position,
        }
    }
}
