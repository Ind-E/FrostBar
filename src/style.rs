use iced::{
    Background, Border, Color, Theme,
    border::{self, rounded, top_right},
    widget::{
        container,
        scrollable::{self, Rail, Scroller, Status},
    },
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

pub fn no_rail(_theme: &Theme, _status: Status) -> scrollable::Style {
    let no_rail = Rail {
        background: None,
        border: border::rounded(0),
        scroller: Scroller {
            color: Color::TRANSPARENT,
            border: border::rounded(0),
        },
    };

    scrollable::Style {
        container: container::Style::default(),
        vertical_rail: no_rail,
        horizontal_rail: no_rail,
        gap: None,
    }
}
