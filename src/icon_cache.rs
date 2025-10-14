use freedesktop_icons::{default_theme_gtk, lookup};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use tracing::{debug, error, warn};

use freedesktop_desktop_entry::{DesktopEntry, default_paths};
use iced::widget::{
    image::{self},
    svg,
};

const ICON_SIZE: u16 = 48;
const ICON_SCALE: u16 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Icon {
    Svg(svg::Handle),
    Raster(image::Handle),
}

#[profiling::function]
pub fn client_icon_path(
    app_id: &str,
    icon_theme: Option<&str>,
) -> Option<PathBuf> {
    if let Some(path) = find_icon_from_desktop_file(app_id, icon_theme) {
        debug!("found icon from desktop file");
        Some(path)
    } else if let Some(path) = try_icon_themes(app_id, icon_theme) {
        debug!("found icon from app id");
        Some(path)
    } else {
        error!("icon not found for `{app_id}`");
        None
    }
}

fn find_icon_from_desktop_file(
    app_id: &str,
    icon_theme: Option<&str>,
) -> Option<PathBuf> {
    for path in default_paths().chain(std::iter::once("hi".into())) {
        let desktop_file_path = path.join(format!("{app_id}.desktop"));

        if let Ok(entry) =
            DesktopEntry::from_path(&desktop_file_path, None::<&[&str]>)
            && let Some(icon_path) = entry
                .icon()
                .and_then(|name| try_icon_themes(name, icon_theme))
        {
            return Some(icon_path);
        }
    }
    None
}

fn try_icon_themes(
    icon_path: &str,
    icon_theme: Option<&str>,
) -> Option<PathBuf> {
    if let Some(theme) = icon_theme
        && let Some(icon) = lookup(icon_path)
            .with_theme(theme)
            .with_size(ICON_SIZE)
            .with_scale(ICON_SCALE)
            .find()
    {
        return Some(icon);
    } else if let Some(theme) = default_theme_gtk()
        && let Some(icon) = lookup(icon_path)
            .with_theme(&theme)
            .with_size(48)
            .with_scale(2)
            .find()
    {
        debug!("detected gtk theme {theme}");
        return Some(icon);
    } else if let Some(icon) =
        lookup(icon_path).with_size(48).with_scale(2).find()
    {
        return Some(icon);
    } else if icon_path.contains("steam_app_")
        && let Some(steam_icon) =
            lookup(dbg!(&icon_path.replace("steam_app", "steam_icon")))
                .with_size(48)
                .with_scale(2)
                .find()
    {
        return Some(steam_icon);
    }

    None
}

#[derive(Debug)]
pub struct IconCache {
    inner: BTreeMap<String, Icon>,
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
            inner: BTreeMap::new(),
        }
    }

    pub fn get_icon(
        &mut self,
        app_id: &str,
        icon_theme: Option<&str>,
    ) -> Option<Icon> {
        if let Some(icon) = self.inner.get(app_id) {
            return Some(icon.clone());
        }

        let icon = client_icon_path(app_id, icon_theme)
            .and_then(|path| load_icon_from_path(&path))?;

        self.inner.insert(app_id.to_string(), icon.clone());
        Some(icon)
    }
}
