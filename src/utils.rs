use iced::futures::Stream;
use std::pin::Pin;

pub mod log;
pub mod style;
pub mod view;
pub mod window;

pub type BoxStream<T> = Pin<Box<dyn Stream<Item = T> + Send>>;
