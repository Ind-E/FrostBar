use downcast_rs::{Downcast, impl_downcast};
use iced::{Element, widget::container};

use crate::{Message, config};

pub mod battery;
pub mod cava;
pub mod label;
pub mod mpris;
pub mod niri;
pub mod time;

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub struct BarPosition {
    pub idx: usize,
    pub align: BarAlignment,
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum BarAlignment {
    Start,
    Middle,
    End,
}

pub trait ViewTrait<M>: Downcast {
    fn view<'a>(
        &'a self,
        modules: &'a M,
        layout: &'a config::Layout,
    ) -> Element<'a, Message>;

    fn position(&self) -> BarPosition;

    fn tooltip<'a>(
        &'a self,
        _modules: &'a M,
        _id: &container::Id,
    ) -> Option<Element<'a, Message>> {
        None
    }

    fn synchronize(&mut self, _modules: &M) {}
}
impl_downcast!(ViewTrait<M>);
