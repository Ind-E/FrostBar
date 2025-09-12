pub mod battery;
pub mod cava;
pub mod label;
pub mod mpris;
pub mod niri;
pub mod time;

#[derive(PartialEq, Eq, Debug)]
pub struct BarPosition {
    pub idx: usize,
    pub align: BarAlignment,
}

#[derive(PartialEq, Eq, Debug)]
pub enum BarAlignment {
    Start,
    Middle,
    End,
}
