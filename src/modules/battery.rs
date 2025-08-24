use iced::{
    Element, Length, Padding,
    widget::{Container, Text, container, stack},
};

use crate::{
    Message,
    config::{BATTERY_ICON_SIZE, CHARGING_OVERLAY_SIZE},
    style::styled_tooltip,
};
extern crate starship_battery as battery;

#[derive(Debug, Clone)]
pub struct BatteryInfo {
    percentage: f32,
    state: battery::State,
}

pub struct BatteryModule {
    pub id: container::Id,
    manager: Option<battery::Manager>,
    batteries: Vec<BatteryInfo>,
}

impl BatteryModule {
    pub fn new() -> Self {
        let manager = match battery::Manager::new() {
            Ok(manager) => Some(manager),
            Err(e) => {
                log::error!("{e}");
                None
            }
        };

        Self {
            id: container::Id::unique(),
            manager,
            batteries: Vec::new(),
        }
    }

    pub fn fetch_battery_info(&mut self) {
        let manager = match &self.manager {
            Some(manager) => manager,
            Option::None => return log::error!("No battery manager"),
        };

        let batteries = match manager.batteries() {
            Ok(batteries) => batteries,
            Err(e) => return log::error!("{e}"),
        };

        let mut info = Vec::with_capacity(2);
        for battery in batteries {
            let mut bat = match battery {
                Ok(bat) => bat,
                Err(e) => return log::error!("{e}"),
            };

            if let Err(e) = manager.refresh(&mut bat) {
                return log::error!("{e}");
            }

            info.push(BatteryInfo {
                percentage: (bat.energy() / bat.energy_full()).into(),
                state: bat.state(),
            });
        }

        self.batteries = info;
    }

    pub fn to_widget<'a>(&self) -> Element<'a, Message> {
        if self.batteries.is_empty() {
            log::warn!("No batteries found to display");
            return Text::new("?").size(CHARGING_OVERLAY_SIZE).into();
        }

        let total_percentage: f32 = self.batteries.iter().map(|b| b.percentage).sum();
        let avg_percentage = total_percentage / self.batteries.len() as f32;

        let icon = get_battery_icon(avg_percentage);

        let is_charging = self.batteries.iter().all(|b| {
            !matches!(b.state, battery::State::Discharging | battery::State::Empty)
        });

        let icon_widget = Container::new(Text::new(icon).size(BATTERY_ICON_SIZE))
            .center_x(Length::Fill)
            .id(self.id.clone());

        let content: Element<'a, Message> = if is_charging {
            let charging_overlay =
                Container::new(Text::new("󱐋").size(CHARGING_OVERLAY_SIZE))
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
            self.batteries
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

fn get_battery_icon(percentage: f32) -> &'static str {
    match percentage {
        p if !(0.0..=1.0).contains(&p) => {
            log::warn!("Battery percentage {} is out of range [0.0, 1.0]", p);
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
