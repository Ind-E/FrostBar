use iced::{
    Element, Length, Padding,
    widget::{Container, Text, container, stack},
};
use tracing::warn;

use crate::{
    Message, config::Config, services::battery::BatteryService, style::styled_tooltip,
};
extern crate starship_battery as battery;

pub struct BatteryView {
    pub id: container::Id,
}

impl<'a> BatteryView {
    pub fn view(
        &self,
        service: &BatteryService,
        config: &Config,
    ) -> Element<'a, Message> {
        let config = &config.modules.battery;
        if service.batteries.is_empty() {
            warn!("No batteries found to display");
            return Text::new("?").size(config.overlay_icon_size).into();
        }

        let total_percentage: f32 = service.batteries.iter().map(|b| b.percentage).sum();
        let avg_percentage = total_percentage / service.batteries.len() as f32;

        let icon = get_battery_icon(avg_percentage);

        let is_charging = service.batteries.iter().all(|b| {
            !matches!(b.state, battery::State::Discharging | battery::State::Empty)
        });

        let icon_widget = Container::new(Text::new(icon).size(config.icon_size))
            .center_x(Length::Fill)
            .id(self.id.clone());

        let content: Element<'a, Message> = if is_charging {
            let charging_overlay =
                Container::new(Text::new("󱐋").size(config.overlay_icon_size))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .padding(Padding {
                        top: 7.0,
                        left: 27.0,
                        right: 0.0,
                        bottom: 0.0,
                    });
            stack![icon_widget, charging_overlay].into()
        } else {
            icon_widget.into()
        };

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

        styled_tooltip(content, tooltip)
    }
}

impl BatteryView {
    pub fn new() -> Self {
        Self {
            id: container::Id::unique(),
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
