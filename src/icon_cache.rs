use base64::Engine;
use freedesktop_icons::lookup;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use system_tray::item::StatusNotifierItem;

use base64::engine::general_purpose;
use freedesktop_desktop_entry::{DesktopEntry, default_paths};
use iced::widget::{
    image::{self, Handle},
    svg,
};
use xdgkit::icon_finder;

use crate::config::ICON_THEME;

pub const DEFAULT_ICON: &str =
    "/usr/share/icons/Adwaita/16x16/apps/help-contents-symbolic.symbolic.png";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Icon {
    Svg(svg::Handle),
    Raster(image::Handle),
}

pub fn client_icon_path(
    app_id: &str,
) -> Result<PathBuf, freedesktop_desktop_entry::DecodeError> {
    let mut paths = default_paths();

    let desktop_file = paths
        .find_map(|p| {
            let file = p.join(&format!("{}.desktop", app_id));
            if file.exists() { Some(file) } else { None }
        })
        .map(
            |df| -> Result<
                Option<PathBuf>,
                freedesktop_desktop_entry::DecodeError,
            > {
                let content = std::fs::read_to_string(&df)?;

                let entry =
                    DesktopEntry::from_str(&df, &content, None::<&[&str]>)?;

                Ok(entry.desktop_entry("Icon").and_then(|icon_name| {
                    icon_finder::find_icon(icon_name.to_string(), 128, 1)
                }))
            },
        )
        .transpose()?
        .unwrap_or_else(|| {
            icon_finder::find_icon("default-application".to_string(), 128, 1)
        })
        .unwrap_or_else(|| PathBuf::from(DEFAULT_ICON));

    Ok(desktop_file)
}

pub struct IconCache {
    inner: BTreeMap<String, Option<Icon>>,
}

fn load_icon_from_path(path: &Path) -> Option<Icon> {
    match path.extension().and_then(|s| s.to_str()) {
        Some("svg") => Some(Icon::Svg(svg::Handle::from_path(&path))),
        Some("png") | Some("jpg") => {
            Some(Icon::Raster(image::Handle::from_path(&path)))
        }
        _ => {
            eprintln!(
                "Warning: Unrecognized or missing icon extension at path: {path:?}"
            );
            None
        }
    }
}

impl IconCache {
    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    pub fn get_icon(&mut self, app_id: &str) -> &Option<Icon> {
        self.inner.entry(app_id.to_string()).or_insert_with(|| {
            client_icon_path(app_id)
                .ok()
                .and_then(|path| load_icon_from_path(&path))
        })
    }

    pub fn get_tray_icon(&mut self, item: &StatusNotifierItem) -> &Option<Icon> {
        let key = match &item.icon_name {
            Some(name) if !name.is_empty() => name.clone(),
            _ => item.id.clone(),
        };
        self.inner.entry(key).or_insert_with(|| {
            if let Some(icon_name) = &item.icon_name {
                lookup(icon_name)
                    .with_theme(ICON_THEME)
                    .with_scale(2)
                    .find()
                    .and_then(|path| load_icon_from_path(&path))
                    .or_else(|| {
                        lookup(icon_name)
                            .find()
                            .and_then(|path| load_icon_from_path(&path))
                    })
            } else {
                None
            }
            .or_else(|| {
                item.icon_pixmap.as_ref().and_then(|pixmaps| {
                    pixmaps.iter().max_by_key(|p| p.width).map(|pixmap| {
                        Icon::Raster(Handle::from_rgba(
                            pixmap.width as u32,
                            pixmap.height as u32,
                            pixmap.pixels.clone(),
                        ))
                    })
                })
            })
        })
    }
}

pub struct MprisArtCache {
    inner: BTreeMap<String, Option<image::Handle>>,
}

impl MprisArtCache {
    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    pub fn get_art(&mut self, art_url: &str) -> &Option<image::Handle> {
        self.inner.entry(art_url.to_string()).or_insert_with(|| {
            if let Some(url) = art_url.strip_prefix("data:image/jpeg;base64,") {
                let image_bytes = match general_purpose::STANDARD.decode(url) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        eprintln!("icon_cache get_art error: {e}");
                        return None;
                    }
                };
                Some(image::Handle::from_bytes(image_bytes))
            } else if let Some(url) = art_url.strip_prefix("file://") {
                Some(image::Handle::from_path(url))
            } else {
                None
            }
        })
    }
}
