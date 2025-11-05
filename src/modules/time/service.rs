use chrono::{DateTime, Local};
use iced::{
    Subscription,
    time::{self, Duration},
};

use crate::{Message, modules};

pub struct TimeService {
    pub time: DateTime<Local>,
}

#[profiling::all_functions]
impl TimeService {
    pub fn new() -> Self {
        Self { time: Local::now() }
    }

    pub fn subscription() -> Subscription<Message> {
        time::every(Duration::from_secs(1))
            .map(|_| Message::Module(modules::ModuleMsg::Tick(Local::now())))
    }

    pub fn handle_event(&mut self, event: DateTime<Local>) {
        self.time = event;
    }
}
