use iced::{
    Element, Length,
    widget::{Container, Text, container, text},
};

use crate::{
    Message, config,
    services::time::TimeService,
    style::{container_style, styled_tooltip},
    utils::maybe_mouse_binds,
    views::BarPosition,
};

pub struct TimeView {
    pub id: container::Id,
    config: config::Time,
    pub position: BarPosition,
}

#[profiling::all_functions]
impl TimeView {
    pub fn view(
        &self,
        service: &TimeService,
        layout: &config::Layout,
    ) -> Element<'_, Message> {
        let time = service.time.format(&self.config.format).to_string();
        let tooltip = Text::new(
            service.time.format(&self.config.tooltip_format).to_string(),
        );

        let mut content = Container::new(text(time).size(16))
            .id(self.id.clone())
            .style(container_style(&self.config.style));

        if layout.anchor.vertical() {
            content = content.center_x(Length::Fill);
        } else {
            content = content.center_y(Length::Fill);
        }

        let content = maybe_mouse_binds(content, &self.config.binds);

        styled_tooltip(content, tooltip, layout.anchor)
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
