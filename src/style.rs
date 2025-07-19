use iced::{
    Background, Border, Color, Theme,
    border::{Radius, rounded, top_right},
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
            width: 0.0,
            ..Default::default()
        },
        ..Default::default()
    })
}

pub fn workspace_style<'a>(
    active: bool,
    hovered: bool,
) -> container::StyleFn<'a, Theme> {
    Box::new(move |_| container::Style {
        border: Border {
            color: if active {
                Color::WHITE
            } else {
                Color::from_rgb(0.3, 0.3, 0.3)
            },
            width: 2.0,
            radius: Radius::new(12),
        },
        background: Some(Background::Color(if hovered {
            Color::from_rgba(0.8, 0.8, 0.8, 0.015)
        } else {
            Color::TRANSPARENT
        })),
        ..Default::default()
    })
}
