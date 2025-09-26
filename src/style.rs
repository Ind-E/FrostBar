use iced::{
    Background, Color, Element, Theme,
    border::rounded,
    widget::{Container, Tooltip, container, tooltip::Position},
};

use crate::{Message, config};

pub fn _bg(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba(
            0.0, 0.0, 0.0, 0.8,
        ))),
        ..Default::default()
    }
}

pub fn styled_tooltip<'a>(
    content: impl Into<Element<'a, Message>>,
    tooltip: impl Into<Element<'a, Message>>,
    anchor: &config::Anchor,
) -> Element<'a, Message> {
    Tooltip::new(
        content,
        Container::new(tooltip)
            .style(|_theme| container::Style {
                background: Some(Background::Color(Color::from_rgba(
                    0.0, 0.0, 0.0, 0.8,
                ))),

                ..Default::default()
            })
            .padding(5),
        match anchor {
            config::Anchor::Left => Position::Right,
            config::Anchor::Right => Position::Left,
            config::Anchor::Top => Position::Bottom,
            config::Anchor::Bottom => Position::Top,
        },
    )
    .into()
}

pub fn workspace_style<'a>(
    active: bool,
    hovered: bool,
    radius: u16,
) -> container::StyleFn<'a, Theme> {
    let mut base = container::Style::default();
    if hovered {
        base = base.background(Color::from_rgba(0.25, 0.25, 0.25, 0.2));
    }
    if active {
        base = base.border(rounded(radius).color(Color::WHITE).width(2));
    }
    Box::new(move |_| base)
}
