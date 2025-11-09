use crate::{
    Element, config,
    modules::{BarPosition, Modules, ViewTrait, mouse_binds},
    utils::style::container_style,
};
use iced::{
    Length,
    widget::{Column, Container, Text, container},
};
use std::any::Any;
use tracing::warn;
extern crate starship_battery as battery;

pub struct BatteryView {
    pub id: container::Id,
    config: config::Battery,
    pub position: BarPosition,
}

#[profiling::all_functions]
impl ViewTrait<Modules> for BatteryView {
    fn view<'a>(
        &'a self,
        service: &'a Modules,
        layout: &'a config::Layout,
    ) -> Element<'a> {
        let service = &service.battery;
        if service.is_empty {
            return Column::new().into();
        }

        let icon = get_battery_icon(service.avg_percentage);

        let icon_text = if service.is_charging {
            Text::new(icon)
                .size(self.config.icon_size)
                .color(self.config.charging_color)
        } else {
            Text::new(icon).size(self.config.icon_size)
        };

        let mut icon_widget = Container::new(icon_text);
        icon_widget = container_style(icon_widget, &self.config.style, layout)
            .id(self.id.clone());

        if layout.anchor.vertical() {
            icon_widget = icon_widget.center_x(Length::Fill);
        } else {
            icon_widget = icon_widget.center_y(Length::Fill);
        }

        mouse_binds(icon_widget, &self.config.binds, Some(self.id.clone()))
    }

    fn position(&self) -> BarPosition {
        self.position
    }

    fn tooltip<'a>(
        &'a self,
        modules: &'a Modules,
        id: &container::Id,
    ) -> Option<Element<'a>> {
        if *id != self.id {
            return None;
        }
        let battery = &modules.battery;
        Some(
            Text::new(
                battery
                    .batteries
                    .iter()
                    .enumerate()
                    .map(|(i, bat)| {
                        format!(
                            "Battery {}: {}% ({})",
                            i + 1,
                            (bat.percentage * 100.0).floor(),
                            bat.state
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
            .into(),
        )
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[profiling::all_functions]
impl BatteryView {
    pub fn new(config: config::Battery, position: BarPosition) -> Self {
        Self {
            id: container::Id::unique(),
            config,
            position,
        }
    }
}

#[profiling::function]
fn get_battery_icon(percentage: f32) -> &'static str {
    match percentage {
        p if !(0.0..=1.0).contains(&p) => {
            warn!(target: "battery", "Battery percentage {} is out of range [0.0, 1.0]", p);
            "?"
        }
        p if p < 0.1 => "󰁺",
        p if p < 0.2 => "󰁻",
        p if p < 0.3 => "󰁼",
        p if p < 0.4 => "󰁽",
        p if p < 0.5 => "󰁾",
        p if p < 0.6 => "󰁿",
        p if p < 0.7 => "󰂀",
        p if p < 0.8 => "󰂁",
        p if p < 0.9 => "󰂂",
        p if p <= 1.0 => "󰁹",
        _ => unreachable!(),
    }
}
