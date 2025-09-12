use iced::{
    Element, Length,
    widget::{Container, Text, container, text},
};

use crate::{Message, config, style::styled_tooltip};

pub struct LabelView {
    pub id: container::Id,
}

impl<'a> LabelView {
    pub fn view(&self, config: &config::Label) -> Element<'a, Message> {
        let content = Container::new(text(config.text.clone()).size(16))
            .center_x(Length::Fill)
            .id(self.id.clone());

        if let Some(tooltip) = &config.tooltip {
            let tooltip = Text::new(tooltip.clone());
            styled_tooltip(content, tooltip)
        } else {
            content.into()
        }
    }
}

impl LabelView {
    pub fn new() -> Self {
        Self {
            id: container::Id::unique(),
        }
    }
}
