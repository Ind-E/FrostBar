use iced::time::{self, Duration};
use tracing::error;

use crate::{Message, services::Service};
extern crate starship_battery as battery;

#[derive(Debug, Clone)]
pub struct BatteryInfo {
    pub percentage: f32,
    pub state: battery::State,
}

pub struct BatteryService {
    pub manager: Option<battery::Manager>,
    pub batteries: Vec<BatteryInfo>,
}

impl Service for BatteryService {
    type Event = ();
    fn handle_event(&mut self, _event: Self::Event) -> iced::Task<Message> {
        self.fetch_battery_info();
        iced::Task::none()
    }

    fn subscription() -> iced::Subscription<Message> {
        time::every(Duration::from_secs(1)).map(|_| Message::UpdateBattery)
    }
}

impl BatteryService {
    pub fn new() -> Self {
        let manager = match battery::Manager::new() {
            Ok(manager) => Some(manager),
            Err(e) => {
                error!("{e}");
                None
            }
        };

        Self {
            manager,
            batteries: Vec::new(),
        }
    }
    fn fetch_battery_info(&mut self) {
        let manager = match &self.manager {
            Some(manager) => manager,
            Option::None => return error!("No battery manager"),
        };

        let batteries = match manager.batteries() {
            Ok(batteries) => batteries,
            Err(e) => return error!("{e}"),
        };

        let mut info = Vec::with_capacity(2);
        for battery in batteries {
            let mut bat = match battery {
                Ok(bat) => bat,
                Err(e) => return error!("{e}"),
            };

            if let Err(e) = manager.refresh(&mut bat) {
                return error!("{e}");
            }

            info.push(BatteryInfo {
                percentage: (bat.energy() / bat.energy_full()).into(),
                state: bat.state(),
            });
        }

        self.batteries = info;
    }
}
