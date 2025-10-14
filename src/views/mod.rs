use crate::views::{
    battery::BatteryView, cava::CavaView, label::LabelView, mpris::MprisView,
    niri::NiriView, time::TimeView,
};

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

pub enum View {
    Battery(BatteryView),
    Cava(CavaView),
    Label(LabelView),
    Mpris(MprisView),
    Niri(NiriView),
    Time(TimeView),
}
