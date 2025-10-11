use iced::{
    Background, Color, Element, Theme,
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
    anchor: config::Anchor,
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
    style: &config::ContainerStyle,
) -> container::StyleFn<'a, Theme> {
    let mut base = container::Style::default();
    if hovered {
        base = base.background(Color::from_rgba(0.4, 0.4, 0.4, 0.4));
    }
    if active {
        return container_style(style);
    }
    Box::new(move |_| base)
}

pub fn container_style<'a>(
    style: &config::ContainerStyle,
) -> container::StyleFn<'a, Theme> {
    let retval = container::Style {
        text_color: style.text_color.as_ref().map(Into::into),
        background: style
            .background
            .as_ref()
            .map(|b| Background::Color(b.into())),
        border: style.border.clone().unwrap_or_default().into(),
        ..Default::default()
    };

    Box::new(move |_| retval)
}
