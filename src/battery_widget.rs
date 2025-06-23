use iced::{
    Element, Length, Padding,
    widget::{MouseArea, container, stack, text},
};
use std::sync::LazyLock;
use std::sync::Mutex;
extern crate starship_battery as battery;

use crate::{Message, MouseEnterEvent};
static BATTERY_MANAGER: LazyLock<Mutex<Result<battery::Manager, battery::Error>>> =
    LazyLock::new(|| Mutex::new(battery::Manager::new()));

#[derive(Debug, Clone)]
pub struct BatteryInfo {
    pub percentage: f32,
    pub state: battery::State,
}

pub fn fetch_battery_info() -> Message {
    let manager_guard = BATTERY_MANAGER.lock().unwrap();
    let manager: &battery::Manager = match manager_guard.as_ref() {
        Ok(manager) => manager,
        Err(e) => return err(e),
    };

    let batteries = match manager.batteries() {
        Ok(batteries) => batteries,
        Err(e) => return err(&e),
    };

    let mut info = Vec::with_capacity(2);
    for battery in batteries {
        if let Ok(mut bat) = battery {
            info.push(BatteryInfo {
                percentage: (bat.energy() / bat.energy_full()).into(),
                state: bat.state(),
            });
            if let Err(e) = manager.refresh(&mut bat) {
                return err(&e);
            }
        }
    }

    Message::BatteryUpdate(info)
}

fn err(e: &battery::Error) -> Message {
    Message::ErrorMessage(e.to_string())
}

pub fn battery_icon<'a>(
    info: Option<&Vec<BatteryInfo>>,
    id: container::Id,
) -> Element<'a, Message> {
    if info.is_none() {
        return stack![].into();
    }
    let info = info.as_ref().unwrap();
    let percentage = info.iter().map(|b| b.percentage).sum::<f32>() / info.len() as f32;
    let icon = match percentage {
        x if x < 0.0 => "?",
        x if x < 0.1 => "󰁺",
        x if x < 0.2 => "󰁻",
        x if x < 0.3 => "󰁼",
        x if x < 0.4 => "󰁽",
        x if x < 0.5 => "󰁾",
        x if x < 0.6 => "󰁿",
        x if x < 0.7 => "󰂀",
        x if x < 0.8 => "󰂁",
        x if x < 0.9 => "󰂂",
        x if x <= 1.0 => "󰁹",
        _ => "?",
    };

    let charging: bool = info.iter().all(|b| match b.state {
        battery::State::Unknown | battery::State::Charging | battery::State::Full => true,
        battery::State::Discharging | battery::State::Empty => false,
    });

    let icon_widget = text(icon).size(22);

    if charging {
        MouseArea::new(stack![
            container(icon_widget).center_x(Length::Fill).id(id.clone()),
            container(text("󱐋").size(13))
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(Padding {
                    top: 7.0,
                    left: 27.0,
                    right: 0.0,
                    bottom: 0.0
                })
        ])
        .on_enter(Message::MouseEntered(MouseEnterEvent::Tooltip(id.clone())))
        .on_exit(Message::MouseExited(MouseEnterEvent::Tooltip(id)))
        .into()
    } else {
        icon_widget.into()
    }
}
