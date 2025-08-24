use iced::{
    Font,
    font::{Family, Weight},
};

pub const BAR_WIDTH: u32 = 42;
pub const GAPS: i32 = 3;
pub const VOLUME_PERCENT: i32 = 3;
pub const BORDER_RADIUS: u16 = 4;

pub const FIRA_CODE_BYTES: &[u8] =
    include_bytes!("../assets/FiraCodeNerdFontMono-Medium.ttf");
pub const FIRA_CODE: Font = Font {
    family: Family::Name("FiraCode Nerd Font Mono"),
    weight: Weight::Medium,
    ..Font::DEFAULT
};

pub const ICON_THEME: &str = "Flat-Remix-Blue-Dark";

pub const BAR_NAMESPACE: &str = "FrostBar";

pub const BATTERY_ICON_SIZE: u32 = 22;
pub const CHARGING_OVERLAY_SIZE: u32 = 13;

pub const CAVA_BAR_SPACING_PERCENT: f32 = 0.1;
pub const CAVA_BARS: usize = 10;
