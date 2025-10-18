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
    active_style: &config::ContainerStyle,
    hovered_style: &config::ContainerStyle,
    base_style: &config::ContainerStyle,
) -> container::StyleFn<'a, Theme> {
    let mut style = container::Style::default();
    if let Some(text_color) = &base_style.text_color {
        style.text_color = Some(text_color.into());
    }
    if let Some(background) = &base_style.background {
        style.background = Some(Background::Color(background.into()));
    }
    if let Some(border) = &base_style.border {
        if let Some(color) = &border.color {
            style.border.color = color.into();
        }
        if let Some(width) = border.width {
            style.border.width = width;
        }
        if let Some(radius) = &border.radius {
            style.border.radius = radius.clone().into();
        }
    }
    if hovered {
        if let Some(text_color) = &hovered_style.text_color {
            style.text_color = Some(text_color.into());
        };
        if let Some(background) = &hovered_style.background {
            style.background = Some(Background::Color(background.into()));
        }

        if let Some(border) = &hovered_style.border {
            if let Some(color) = &border.color {
                style.border.color = color.into();
            }
            if let Some(width) = border.width {
                style.border.width = width;
            }
            if let Some(radius) = &border.radius {
                style.border.radius = radius.clone().into();
            }
        }
    }
    if active {
        if let Some(text_color) = &active_style.text_color {
            style.text_color = Some(text_color.into());
        };
        if let Some(background) = &active_style.background {
            style.background = Some(Background::Color(background.into()));
        }

        if let Some(border) = &active_style.border {
            if let Some(color) = &border.color {
                style.border.color = color.into();
            }
            if let Some(width) = border.width {
                style.border.width = width;
            }
            if let Some(radius) = &border.radius {
                style.border.radius = radius.clone().into();
            }
        }
    }
    Box::new(move |_| style)
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
