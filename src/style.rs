use iced::{
    Background, Border, Color, Theme,
    border::{rounded, top_right},
    widget::container,
};

pub fn rounded_corners(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.8))),
        border: rounded(12),
        ..Default::default()
    }
}

pub fn tooltip_style<'a>(opacity: f32) -> container::StyleFn<'a, Theme> {
    Box::new(move |_| container::Style {
        background: Some(Background::Color(Color::from_rgba(
            0.0,
            0.0,
            0.0,
            opacity * 0.8,
        ))),
        border: Border {
            radius: top_right(12).bottom_right(12),
            width: 0.928,
            ..Default::default()
        },
        ..Default::default()
    })
}
