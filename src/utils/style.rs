use iced::{
    Theme,
    padding::{left, top},
    widget::{Container, container},
};

use crate::{
    Message,
    config::{self, NiriWindowStyle, NiriWorkspaceStyle},
};

pub fn workspace_style(
    active: bool,
    hovered: bool,
    style: &NiriWorkspaceStyle,
) -> container::StyleFn<'_, Theme> {
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

pub fn window_style(
    focused: bool,
    style: &NiriWindowStyle,
) -> container::StyleFn<'_, Theme> {
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
