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
    pub avg_percentage: f32,
    pub is_charging: bool,
    pub is_empty: bool,
}

#[profiling::all_functions]
impl BatteryService {
    pub fn new() -> Self {
        let manager = match battery::Manager::new() {
            Ok(manager) => Some(manager),
            Err(e) => {
                error!("battery: {e}");
                None
            }
        };
        let mut new = Self {
            manager,
            batteries: Vec::new(),
            avg_percentage: 0.0,
            is_charging: false,
            is_empty: true,
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
            Err(e) => return error!("battery: {e}"),
        };

        let mut info = Vec::with_capacity(2);
        for battery in batteries {
            let mut bat = match battery {
                Ok(bat) => bat,
                Err(e) => return error!("battery: {e}"),
            };

            if let Err(e) = manager.refresh(&mut bat) {
                return error!("battery: {e}");
            }

            info.push(BatteryInfo {
                percentage: (bat.energy() / bat.energy_full()).into(),
                state: bat.state(),
            });
        }

        self.batteries = info;

        let total_percentage: f32 =
            self.batteries.iter().map(|b| b.percentage).sum();
        self.avg_percentage = total_percentage / self.batteries.len() as f32;

        self.is_charging = self.batteries.iter().all(|b| {
            !matches!(
                b.state,
                battery::State::Discharging | battery::State::Empty
            )
        });

        self.is_empty = self.batteries.is_empty();
    }
}
