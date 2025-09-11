use iced::{Subscription, futures::Stream};
use std::pin::Pin;

use crate::Message;

pub mod battery;
pub mod cava;
pub mod mpris;
pub mod niri;
pub mod systray;
pub mod time;

pub type EventStream<T, E> = Pin<Box<dyn Stream<Item = Result<T, E>> + Send>>;

pub trait Service {

    fn subscription() -> Subscription<Message>;

    type Event;
    fn handle_event(&mut self, event: Self::Event) -> iced::Task<Message>;
}
