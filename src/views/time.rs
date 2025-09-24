use iced::{
    Element, Length,
    widget::{Container, Text, container, text},
};

use crate::{
    Message, config, services::time::TimeService, style::styled_tooltip,
    utils::maybe_mouse_binds, views::BarPosition,
};

pub struct TimeView {
    pub id: container::Id,
    config: config::Time,
    pub position: BarPosition,
}

#[profiling::all_functions]
impl TimeView {
    pub fn view(&self, service: &TimeService) -> Element<'_, Message> {
        let time = service.time.format(&self.config.format).to_string();
        let tooltip =
            Text::new(service.time.format(&self.config.tooltip_format).to_string());
        let content = Container::new(text(time).size(16))
            .center_x(Length::Fill)
            .id(self.id.clone());

        let content = maybe_mouse_binds(content, &self.config.binds);

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
