use iced_layershell::{
    actions::IcedNewPopupSettings,
    reexport::{
        Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings,
        OutputOption,
    },
};

use crate::{config, Message, BAR_NAMESPACE};

#[profiling::function]
pub fn open_window(
    layout: &config::Layout,
) -> (iced::window::Id, iced::Task<Message>) {
    let size = Some(match layout.anchor {
        config::Anchor::Left | config::Anchor::Right => (layout.width, 0),
        config::Anchor::Top | config::Anchor::Bottom => (0, layout.width),
    });

    let anchor = match layout.anchor {
        config::Anchor::Left => Anchor::Left | Anchor::Top | Anchor::Bottom,
        config::Anchor::Right => Anchor::Right | Anchor::Top | Anchor::Bottom,
        config::Anchor::Top => Anchor::Top | Anchor::Left | Anchor::Right,
        config::Anchor::Bottom => Anchor::Bottom | Anchor::Left | Anchor::Right,
    };

    // top, right, bottom, left
    let margin = Some((layout.gaps, layout.gaps, layout.gaps, layout.gaps));

    let layer = match layout.layer {
        config::Layer::Background => Layer::Background,
        config::Layer::Bottom => Layer::Bottom,
        config::Layer::Top => Layer::Top,
        config::Layer::Overlay => Layer::Overlay,
    };

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

    let msg = Message::NewPopUp {
        settings: IcedNewPopupSettings {
            size: (400, 400),
            position: (0, 0),
        },
        id,
    };

    let task = iced::Task::done(msg);

    (id, task)
}
