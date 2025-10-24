use iced::{
    Background, Color, Theme,
    padding::{left, top},
    widget::{Container, container},
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

pub fn workspace_style<'a>(
    active: bool,
    hovered: bool,
    active_style: &config::ContainerStyle,
    hovered_style: &config::ContainerStyle,
    base_style: &config::ContainerStyle,
) -> container::StyleFn<'a, Theme> {
    let mut style = container::Style::default();
    if let Some(text_color) = &base_style.text_color {
        style.text_color = Some(**text_color);
    }
    if let Some(background) = &base_style.background {
        style.background = Some(Background::Color(**background));
    }
    if let Some(border) = &base_style.border {
        if let Some(color) = &border.color {
            style.border.color = **color;
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
            style.text_color = Some(**text_color);
        }
        if let Some(background) = &hovered_style.background {
            style.background = Some(Background::Color(**background));
        }

        if let Some(border) = &hovered_style.border {
            if let Some(color) = &border.color {
                style.border.color = **color;
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
            style.text_color = Some(**text_color);
        }
        if let Some(background) = &active_style.background {
            style.background = Some(Background::Color(**background));
        }

        if let Some(border) = &active_style.border {
            if let Some(color) = &border.color {
                style.border.color = **color;
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
    container: Container<'a, Message>,
    style: &'a config::ContainerStyle,
    layout: &'a config::Layout,
) -> Container<'a, Message> {
    let retval = container.style(move |_| container::Style {
        text_color: style.text_color.as_ref().map(|x| **x),
        background: style.background.as_ref().map(|b| Background::Color(**b)),
        border: style.border.clone().unwrap_or_default().into(),
        ..Default::default()
    });
    let padding = style.padding.unwrap_or(0f32);
    if layout.anchor.vertical() {
        retval.padding(top(padding).bottom(padding))
    } else {
        retval.padding(left(padding).right(padding))
    }
}
