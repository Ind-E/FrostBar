use iced::{
    Element, Length,
    widget::{Container, Text, container, text},
};

use crate::{Message, config, style::styled_tooltip, views::BarPosition};

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

        if let Some(tooltip) = &self.config.tooltip {
            let tooltip = Text::new(tooltip.clone());
            styled_tooltip(content, tooltip)
        } else {
            content.into()
        }
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
