use chrono::{DateTime, Local};
use iced::{
    Subscription,
    time::{self, Duration},
};

use crate::{Message, services::Service};

pub struct TimeService {
    pub time: DateTime<Local>,
}

#[profiling::all_functions]
impl Service for TimeService {
    fn subscription() -> Subscription<Message> {
        time::every(Duration::from_secs(1)).map(|_| Message::Tick(Local::now()))
    }

    type Event = DateTime<Local>;
    fn handle_event(&mut self, event: Self::Event) -> iced::Task<Message> {
        self.time = event;
        iced::Task::none()
    }
}

impl TimeService {
    pub fn new() -> Self {
        Self { time: Local::now() }
    }
}
