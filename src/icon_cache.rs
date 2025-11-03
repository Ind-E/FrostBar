use dashmap::DashMap;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, LazyLock},
};
use system_tray::item::IconPixmap;
use tracing::{debug, warn};

use freedesktop_desktop_entry::{DesktopEntry, default_paths};
use iced::widget::{
    image::{self},
    svg,
};

const ICON_SIZE: u16 = 48;
const ICON_SCALE: u16 = 2;

static ICON_THEME: LazyLock<Option<String>> =
    LazyLock::new(linicon::get_system_theme);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Icon {
    Svg(svg::Handle),
    Raster(image::Handle),
}

#[profiling::function]
pub fn find_icon_path(app_id: &str) -> Option<PathBuf> {
    let icon_name =
        if let Some(icon_from_desktop_file) = from_desktop_file(app_id) {
            icon_from_desktop_file
        } else {
            app_id.to_owned()
        };

    icon_path_from_name(&icon_name)
}

#[profiling::function]
fn from_desktop_file(app_id: &str) -> Option<String> {
    for path in default_paths() {
        let desktop_file_path = path.join(format!("{app_id}.desktop"));

        if let Ok(entry) =
            DesktopEntry::from_path(&desktop_file_path, None::<&[&str]>)
        {
            return entry.icon().map(ToString::to_string);
        }
    }
    None
}

#[profiling::function]
fn icon_path_from_name(icon_path: &str) -> Option<PathBuf> {
    if let Some(Ok(icon)) = linicon::lookup_icon(icon_path)
        .with_size(ICON_SIZE)
        .with_scale(ICON_SCALE)
        .next()
    {
        debug!("linicon");
        return Some(icon.path);
    } else if let Some(theme) = &*ICON_THEME
        && let Some(icon) = freedesktop_icons::lookup(icon_path)
            .with_theme(theme)
            .with_size(ICON_SIZE)
            .with_scale(ICON_SCALE)
            .find()
    {
        return Some(icon);
    } else if let Some(icon) = freedesktop_icons::lookup(icon_path)
        .with_size(ICON_SIZE)
        .with_scale(ICON_SCALE)
        .find()
    {
        return Some(icon);
    } else if icon_path.contains("steam_app_")
        && let Some(steam_icon) = freedesktop_icons::lookup(
            &icon_path.replace("steam_app", "steam_icon"),
        )
        .with_size(ICON_SIZE)
        .with_scale(ICON_SCALE)
        .find()
    {
        return Some(steam_icon);
    }

    None
}

#[derive(Debug, Clone)]
pub struct IconCache {
    inner: Arc<DashMap<String, Icon>>,
}

#[profiling::function]
fn load_icon_from_path(path: &Path) -> Option<Icon> {
    match path.extension().and_then(|s| s.to_str()) {
        Some("svg") => Some(Icon::Svg(svg::Handle::from_path(path))),
        Some("png" | "jpg") => {
            Some(Icon::Raster(image::Handle::from_path(path)))
        }
        _ => {
            warn!(
                "Unrecognized or missing icon extension at path: {}",
                path.display()
            );
            None
        }
    }
}

#[profiling::all_functions]
impl IconCache {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
        }
    }

    pub fn get_icon(&self, app_id: &str) -> Option<Icon> {
        if let Some(icon) = self.inner.get(app_id) {
            return Some(icon.clone());
        }

        let icon = find_icon_path(app_id)
            .and_then(|path| load_icon_from_path(&path))?;

        self.inner.insert(app_id.to_string(), icon.clone());
        Some(icon)
    }

    pub fn get_tray_icon(
        &self,
        icon_name: Option<String>,
        icon_pixmaps: Option<Vec<IconPixmap>>,
    ) -> Option<Icon> {
        if let Some(icon_name) = icon_name
            && let Some(icon) = self.get_icon(&icon_name)
        {
            Some(icon)
        } else if let Some(icon_pixmaps) = icon_pixmaps {
            largest_icon_from_pixmaps(icon_pixmaps)
        } else {
            None
        }
    }
}

fn largest_icon_from_pixmaps(pixmaps: Vec<IconPixmap>) -> Option<Icon> {
    pixmaps
        .into_iter()
        .max_by(
            |IconPixmap {
                 width: w1,
                 height: h1,
                 ..
             },
             IconPixmap {
                 width: w2,
                 height: h2,
                 ..
             }| { (w1 * h1).cmp(&(w2 * h2)) },
        )
        .map(|IconPixmap { pixels, .. }| {
            let handle = image::Handle::from_bytes(pixels);
            Icon::Raster(handle)
        })
}
