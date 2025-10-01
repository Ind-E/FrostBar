use iced::{
    Element, Length,
    widget::{Container, Text, container, text},
};

use crate::{
    Message, config,
    style::{container_style, styled_tooltip},
    utils::maybe_mouse_binds,
    views::BarPosition,
};

pub struct LabelView {
    pub id: container::Id,
    config: config::Label,
    pub position: BarPosition,
}

#[profiling::all_functions]
impl<'a> LabelView {
    pub fn view(&'a self, layout: &config::Layout) -> Element<'a, Message> {
        let mut content = Container::new(
            text(self.config.text.clone()).size(self.config.size),
        )
        .style(container_style(&self.config.style))
        .id(self.id.clone());

        if layout.anchor.vertical() {
            content = content.center_x(Length::Fill);
        } else {
            content = content.center_y(Length::Fill);
        }

        let element = if let Some(tooltip) = &self.config.tooltip {
            let tooltip = Text::new(tooltip.clone());
            styled_tooltip(content, tooltip, &layout.anchor)
        } else {
            content.into()
        };

        maybe_mouse_binds(element, &self.config.binds)
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
