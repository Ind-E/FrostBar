use crate::config::{BAR_WIDTH, FIRA_CODE, FIRA_CODE_BYTES, GAPS};

use iced_layershell::{
    build_pattern::{MainSettings, daemon},
    reexport::{Anchor, KeyboardInteractivity, Layer},
    settings::{LayerShellSettings, StartMode},
};

use crate::bar::Bar;

mod bar;
mod config;
mod dbus_proxy;
mod icon_cache;
mod modules;
mod style;
mod tooltip;

pub fn main() -> Result<(), iced_layershell::Error> {
    pretty_env_logger::init();
    let (bar, task) = Bar::new();
    daemon(Bar::namespace, Bar::update, Bar::view, Bar::remove_id)
        .subscription(Bar::subscription)
        .style(Bar::style)
        .theme(Bar::theme)
        .settings(MainSettings {
            fonts: vec![FIRA_CODE_BYTES.into()],
            default_font: FIRA_CODE,
            layer_settings: LayerShellSettings {
                size: Some((BAR_WIDTH, 0)),
                exclusive_zone: BAR_WIDTH as i32 - GAPS as i32,
                anchor: Anchor::Left | Anchor::Top | Anchor::Bottom,
                keyboard_interactivity: KeyboardInteractivity::None,
                start_mode: StartMode::Active,
                layer: Layer::Top,
                ..Default::default()
            },
            ..Default::default()
        })
        .run_with(move || (bar, task))
}
