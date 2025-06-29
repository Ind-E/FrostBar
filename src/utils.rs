use chrono::{Local, Timelike};
use iced::{Task, time::Duration};
use tokio::time::sleep;

use crate::bar::Message;
extern crate starship_battery as battery;

pub fn align_clock() -> Task<Message> {
    Task::perform(
        async {
            let now = Local::now();
            let seconds_to_wait = 60 - now.second() as u64;
            sleep(Duration::from_secs(seconds_to_wait)).await;
        },
        |_| Message::AlignClock,
    )
}
