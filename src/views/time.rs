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

pub struct TimeView {
    pub id: container::Id,
    config: config::Time,
    pub position: BarPosition,
}

#[profiling::all_functions]
impl ViewTrait<Modules> for TimeView {
    fn view<'a>(
        &'a self,
        modules: &'a Modules,
        layout: &'a config::Layout,
    ) -> Element<'a, Message> {
        let service = &modules.time;
        let time = service.time.format(&self.config.format).to_string();

        let mut content =
            Container::new(text(time).size(16)).id(self.id.clone());
        content = container_style(content, &self.config.style, layout);

        if layout.anchor.vertical() {
            content = content.center_x(Length::Fill);
        } else {
            content = content.center_y(Length::Fill);
        }

        mouse_binds(content, &self.config.binds, Some(self.id.clone()))
    }

    fn position(&self) -> BarPosition {
        self.position
    }

    fn tooltip<'a>(
        &'a self,
        service: &Modules,
        id: &container::Id,
    ) -> Option<Element<'a, Message>> {
        if *id != self.id {
            return None;
        }
        let service = &service.time;
        Some(
            Text::new(
                service.time.format(&self.config.tooltip_format).to_string(),
            )
            .into(),
        )
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
