use crate::{Message, config};
use iced::{
    Background, Color, Theme,
    padding::{left, top},
    widget::{Container, container},
};

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
    active_hovered_style: &'a config::ContainerStyle,
    active_style: &'a config::ContainerStyle,
    hovered_style: &'a config::ContainerStyle,
    base_style: &'a config::ContainerStyle,
) -> container::StyleFn<'a, Theme> {
    let style = if active && hovered {
        active_hovered_style
    } else if active {
        active_style
    } else if hovered {
        hovered_style
    } else {
        base_style
    };
    Box::new(move |_| style.inner)
}

pub fn container_style<'a>(
    container: Container<'a, Message>,
    style: &'a config::ContainerStyle,
    layout: &'a config::Layout,
) -> Container<'a, Message> {
    let retval = container.style(move |_| style.inner);
    let padding = style.padding.unwrap_or(0f32);
    if layout.anchor.vertical() {
        retval.padding(top(padding).bottom(padding))
    } else {
        retval.padding(left(padding).right(padding))
    }
}
