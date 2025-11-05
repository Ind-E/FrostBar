use iced::{
    Font,
    font::{Family, Weight},
};

// fonts
pub const FIRA_CODE_BYTES: &[u8] =
    include_bytes!("../../assets/FiraCodeNerdFontMono-Medium.ttf");
pub const FIRA_CODE: Font = Font {
    family: Family::Name("FiraCode Nerd Font Mono"),
    weight: Weight::Medium,
    ..Font::DEFAULT
};

pub const BAR_NAMESPACE: &str = "FrostBar";
