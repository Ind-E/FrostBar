use iced::{Background, Color, Theme, border::rounded, widget::container};

use crate::config::BORDER_RADIUS;

pub fn bg(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.8))),
        ..Default::default()
    }
}

pub fn rounded_corners(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.8))),
        border: rounded(BORDER_RADIUS),
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
        // border: Border {
        //     radius: top_right(12).bottom_right(12),
        //     width: 0.0,
        //     ..Default::default()
        // },
        ..Default::default()
    })
}

pub fn workspace_style<'a>(active: bool, hovered: bool) -> container::StyleFn<'a, Theme> {
    let mut base = container::Style::default();
    if hovered {
        base = base.background(Color::from_rgba(0.25, 0.25, 0.25, 0.2))
    };
    if active {
        base = base.border(
            rounded(BORDER_RADIUS)
                .color(Color::WHITE)
                .width(2)
        );
    };
    Box::new(move |_| base)
}
