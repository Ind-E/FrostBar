use std::any::Any;

use iced::{
    Length,
    widget::{Container, Text, container, text},
};

use crate::{
    Element,
    modules::{BarPosition, Modules, ViewTrait, mouse_binds},
    other::config,
    utils::style::container_style,
};

pub struct TimeView {
    pub id: container::Id,
    config: config::Time,
    pub position: BarPosition,
    current_time: String,
    current_tooltip: String,
}

#[profiling::all_functions]
impl ViewTrait<Modules> for TimeView {
    fn view<'a>(
        &'a self,
        _modules: &'a Modules,
        layout: &'a config::Layout,
    ) -> Element<'a> {
        let mut content = Container::new(text(&self.current_time).size(16))
            .id(self.id.clone());
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
        _service: &Modules,
        id: &container::Id,
    ) -> Option<Element<'a>> {
        if *id != self.id {
            return None;
        }
        Some(Text::new(&self.current_tooltip).into())
    }

    fn synchronize(&mut self, modules: &Modules) {
        self.current_time =
            modules.time.time.format(&self.config.format).to_string();

        self.current_tooltip = modules
            .time
            .time
            .format(&self.config.tooltip_format)
            .to_string();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl TimeView {
    pub fn new(config: config::Time, position: BarPosition) -> Self {
        Self {
            id: container::Id::unique(),
            config,
            position,
            current_time: String::new(),
            current_tooltip: String::new(),
        }
    }
}
