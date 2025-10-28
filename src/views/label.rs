use iced::{
    Element, Length,
    widget::{Container, Text, container, text},
};

use crate::{
    Message, config,
    module::Modules,
    style::container_style,
    utils::mouse_binds,
    views::{BarPosition, ViewTrait},
};

pub struct LabelView {
    pub id: container::Id,
    config: config::Label,
    pub position: BarPosition,
}

#[profiling::all_functions]
impl ViewTrait<Modules> for LabelView {
    fn view<'a>(
        &'a self,
        _modules: &'a Modules,
        layout: &'a config::Layout,
    ) -> Element<'a, Message> {
        let mut content = Container::new(
            text(self.config.text.clone()).size(self.config.size),
        );
        content = container_style(content, &self.config.style, layout)
            .id(self.id.clone());

        if layout.anchor.vertical() {
            content = content.center_x(Length::Fill);
        } else {
            content = content.center_y(Length::Fill);
        }

        let tooltip_id = self.config.tooltip.as_ref().map(|_| self.id.clone());

        mouse_binds(content, &self.config.binds, tooltip_id)
    }

    fn position(&self) -> BarPosition {
        self.position
    }

    fn tooltip<'a>(
        &'a self,
        _service: &'a Modules,
        id: &container::Id,
    ) -> Option<Element<'a, Message>> {
        if *id != self.id {
            return None;
        }

        self.config
            .tooltip
            .as_ref()
            .map(|tooltip| Text::new(tooltip.clone()).into())
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
