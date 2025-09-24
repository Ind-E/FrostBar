use iced::{
    Element, Length,
    widget::{Container, Text, container},
};
use tracing::warn;

use crate::{
    Message, config, services::battery::BatteryService, style::styled_tooltip,
    utils::maybe_mouse_binds, views::BarPosition,
};
extern crate starship_battery as battery;

pub struct BatteryView {
    pub id: container::Id,
    config: config::Battery,
    pub position: BarPosition,
}

#[profiling::all_functions]
impl<'a> BatteryView {
    pub fn view(&'a self, service: &BatteryService) -> Element<'a, Message> {
        if service.batteries.is_empty() {
            return Text::new("?").size(self.config.icon_size).into();
        }

        let total_percentage: f32 = service.batteries.iter().map(|b| b.percentage).sum();
        let avg_percentage = total_percentage / service.batteries.len() as f32;

        let icon = get_battery_icon(avg_percentage);

        let is_charging = service.batteries.iter().all(|b| {
            !matches!(b.state, battery::State::Discharging | battery::State::Empty)
        });

        let icon_text = if is_charging {
            Text::new(icon)
                .size(self.config.icon_size)
                .color(&self.config.charging_color)
        } else {
            Text::new(icon).size(self.config.icon_size)
        };

        let icon_widget = Container::new(icon_text)
            .center_x(Length::Fill)
            .id(self.id.clone());

        let tooltip = Text::new(
            service
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
        );

        let icon_widget = maybe_mouse_binds(icon_widget, &self.config.binds);

        styled_tooltip(icon_widget, tooltip)
    }
}

impl BatteryView {
    pub fn new(config: config::Battery, position: BarPosition) -> Self {
        Self {
            id: container::Id::unique(),
            config,
            position,
        }
    }
}

fn get_battery_icon(percentage: f32) -> &'static str {
    match percentage {
        p if !(0.0..=1.0).contains(&p) => {
            warn!("Battery percentage {} is out of range [0.0, 1.0]", p);
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
