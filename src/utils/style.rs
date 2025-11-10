use crate::{
    Message,
    config::{self, NiriWindowStyle, NiriWorkspaceStyle},
};
use iced::{
    Theme,
    padding::{left, top},
    widget::{Container, container},
};

pub fn workspace_style<'a>(
    active: bool,
    hovered: bool,
    style: &'a NiriWorkspaceStyle,
) -> container::StyleFn<'a, Theme> {
    let style = if active && hovered {
        &style.active_hovered
    } else if active {
        &style.active
    } else if hovered {
        &style.hovered
    } else {
        &style.base
    };
    Box::new(move |_| style.inner)
}

pub fn window_style<'a>(
    focused: bool,
    style: &'a NiriWindowStyle,
) -> container::StyleFn<'a, Theme> {
    let style = if focused { &style.focused } else { &style.base };
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
