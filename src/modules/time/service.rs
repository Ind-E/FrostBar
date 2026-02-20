use chrono::{DateTime, Local};
use iced::{
    time::{self, Duration},
    Subscription,
};

use crate::{modules, Message};

pub struct TimeService {
    pub time: DateTime<Local>,
}

#[profiling::all_functions]
impl TimeService {
    pub fn new() -> Self {
        Self { time: Local::now() }
    }

    pub fn subscription() -> Subscription<Message> {
        #[cfg(feature = "tracy")]
        let _ = tracy_client::span!("time sub");
        time::every(Duration::from_secs(1))
            .map(|_| Message::Module(modules::ModuleMsg::Tick(Local::now())))
    }

    pub fn update(&mut self, event: DateTime<Local>) {
        self.time = event;
    }
}
