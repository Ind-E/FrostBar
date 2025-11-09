use crate::{
    Message,
    other::{
        config::{self},
        constants::BAR_NAMESPACE,
    },
};
use iced::{
    Size,
    window::settings::{
        Anchor, KeyboardInteractivity, Layer, LayerShellSettings,
        PlatformSpecific,
    },
};

pub fn open_dummy_window() -> (iced::window::Id, iced::Task<Message>) {
    let (id, open_task) = iced::window::open(iced::window::Settings {
        transparent: true,
        platform_specific: PlatformSpecific {
            layer_shell: LayerShellSettings {
                layer: Some(Layer::Top),
                anchor: Some(
                    Anchor::LEFT | Anchor::TOP | Anchor::BOTTOM | Anchor::RIGHT,
                ),
                input_region: Some((0, 0, 0, 0)),
                keyboard_interactivity: Some(KeyboardInteractivity::None),
                ..Default::default()
            },
            ..Default::default()
        },
        exit_on_close_request: false,
        ..Default::default()
    });

    (id, open_task.map(|_| Message::NoOp))
}

#[profiling::function]
pub fn open_window(
    layout: &config::Layout,
    monitor_size: iced::Size,
) -> (iced::window::Id, iced::Task<Message>) {
    let size = match layout.anchor {
        config::Anchor::Left | config::Anchor::Right => {
            Size::new(layout.width as f32, 0.0)
        }
        config::Anchor::Top | config::Anchor::Bottom => {
            Size::new(0.0, layout.width as f32)
        }
    };

    let anchor = Some(match layout.anchor {
        config::Anchor::Left => Anchor::LEFT | Anchor::TOP | Anchor::BOTTOM,
        config::Anchor::Right => Anchor::RIGHT | Anchor::TOP | Anchor::BOTTOM,
        config::Anchor::Top => Anchor::TOP | Anchor::LEFT | Anchor::RIGHT,
        config::Anchor::Bottom => Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
    });

    // top, right, bottom, left
    let margin = Some((layout.gaps, layout.gaps, layout.gaps, layout.gaps));

    // x, y, width, height
    let input_region = Some(match layout.anchor {
        config::Anchor::Left | config::Anchor::Right => {
            (0, 0, layout.width as i32, monitor_size.height as i32)
        }
        config::Anchor::Top | config::Anchor::Bottom => {
            (0, 0, monitor_size.width as i32, layout.width as i32)
        }
    });

    let layer = Some(match layout.layer {
        config::Layer::Background => Layer::Background,
        config::Layer::Bottom => Layer::Bottom,
        config::Layer::Top => Layer::Top,
        config::Layer::Overlay => Layer::Overlay,
    });

    let (id, open_task) = iced::window::open(iced::window::Settings {
        size,
        decorations: false,
        minimizable: false,
        transparent: true,
        platform_specific: PlatformSpecific {
            layer_shell: LayerShellSettings {
                anchor,
                margin,
                input_region,
                layer,
                exclusive_zone: Some(layout.width as i32 + layout.gaps),
                keyboard_interactivity: Some(KeyboardInteractivity::None),
                namespace: Some(String::from(BAR_NAMESPACE)),
                ..Default::default()
            },
            ..Default::default()
        },
        exit_on_close_request: false,
        ..Default::default()
    });

    (id, open_task.map(|_| Message::NoOp))
}

pub fn open_tooltip_window() -> (iced::window::Id, iced::Task<Message>) {
    let (id, open_task) = iced::window::open(iced::window::Settings {
        transparent: true,
        platform_specific: PlatformSpecific {
            layer_shell: LayerShellSettings {
                layer: Some(Layer::Top),
                anchor: Some(
                    Anchor::LEFT | Anchor::TOP | Anchor::BOTTOM | Anchor::RIGHT,
                ),
                keyboard_interactivity: Some(KeyboardInteractivity::None),
                ..Default::default()
            },
            ..Default::default()
        },
        exit_on_close_request: false,
        ..Default::default()
    });

    (id, open_task.map(|_| Message::NoOp))
}
