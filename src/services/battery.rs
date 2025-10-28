use tracing::error;

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

#[profiling::all_functions]
impl BatteryService {
    pub fn new() -> Self {
        let manager = match battery::Manager::new() {
            Ok(manager) => Some(manager),
            Err(e) => {
                error!("{e}");
                None
            }
        };
        let mut new = Self {
            manager,
            batteries: Vec::new(),
        };

        new.fetch_battery_info();

        new
    }

    pub fn fetch_battery_info(&mut self) {
        let Some(manager) = &self.manager else {
            return error!("No battery manager");
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
