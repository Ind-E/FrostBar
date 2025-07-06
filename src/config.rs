use iced::{
    Font,
    font::{Family, Weight},
};

pub const BAR_WIDTH: u32 = 45;
pub const GAPS: u16 = 3;
pub const VOLUME_PERCENT: i32 = 3;

pub const FIRA_CODE_BYTES: &[u8] =
    include_bytes!("../assets/FiraCodeNerdFontMono-Medium.ttf");
pub const FIRA_CODE: Font = Font {
    family: Family::Name("FiraCode Nerd Font Mono"),
    weight: Weight::Medium,
    ..Font::DEFAULT
};

pub const ICON_THEME: &str = "Flat-Remix-Blue-Dark";

pub const TOOLTIP_RETRIES: u8 = 5;

pub const BAR_NAMESPACE: &str = "Iced Bar";
