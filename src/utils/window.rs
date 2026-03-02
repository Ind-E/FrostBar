use iced_layershell::reexport::{
    Anchor, KeyboardInteractivity, NewLayerShellSettings, OutputOption,
};

use crate::{
    BAR_NAMESPACE, Message,
    config::{self, splat_gaps},
};

#[profiling::function]
pub fn open_window(
    layout: &config::Layout,
) -> (iced::window::Id, iced::Task<Message>) {
    let size = Some(layout.anchor.calc_size(layout.width));

    // top, right, bottom, left
    let margin = Some(splat_gaps(layout.gaps));

    let layer = layout.layer.into();
    let anchor = layout.anchor.into();

    let id = iced::window::Id::unique();

    let msg = Message::NewLayerShell {
        settings: NewLayerShellSettings {
            size,
            layer,
            anchor,
            exclusive_zone: Some(layout.width as i32 + layout.gaps),
            margin,
            keyboard_interactivity: KeyboardInteractivity::None,
            output_option: OutputOption::None,
            events_transparent: false,
            namespace: Some(BAR_NAMESPACE.to_string()),
        },
        id,
    };

    let task = iced::Task::done(msg);

    (id, task)
}

pub fn open_tooltip_window() -> (iced::window::Id, iced::Task<Message>) {
    let id = iced::window::Id::unique();

    let msg = Message::NewLayerShell {
        settings: NewLayerShellSettings {
            anchor: Anchor::all(),
            events_transparent: true,
            keyboard_interactivity: KeyboardInteractivity::None,
            ..Default::default()
        },
        id,
    };

    let task = iced::Task::done(msg);

    (id, task)
}
