use iced::{
    Element, Length,
    widget::{Container, Text, container, text},
};

use crate::{
    Message, config, style::styled_tooltip, utils::maybe_mouse_interaction,
    views::BarPosition,
};

pub struct LabelView {
    pub id: container::Id,
    config: config::Label,
    pub position: BarPosition,
}

impl<'a> LabelView {
    pub fn view(&self) -> Element<'a, Message> {
        let content =
            Container::new(text(self.config.text.clone()).size(self.config.size))
                .center_x(Length::Fill)
                .id(self.id.clone());

        let element = if let Some(tooltip) = &self.config.tooltip {
            let tooltip = Text::new(tooltip.clone());
            styled_tooltip(content, tooltip)
        } else {
            content.into()
        };

        maybe_mouse_interaction(element, &self.config.interaction)
    }
}

impl LabelView {
    pub fn new(config: config::Label, position: BarPosition) -> Self {
        Self {
            id: container::Id::unique(),
            config,
            position,
        }
    }
}
