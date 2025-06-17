use std::{collections::BTreeMap, path::PathBuf};

use color_eyre::{Result, eyre::Context};
use freedesktop_desktop_entry::{DesktopEntry, default_paths};
use iced::widget::svg;
use xdgkit::icon_finder;

pub const DEFAULT_ICON: &str =
    "/usr/share/icons/Adwaita/16x16/apps/help-contents-symbolic.symbolic.png";

//TODO: make this work with svg and (raster) image

pub fn client_icon(app_id: &str) -> Result<PathBuf> {
    let mut paths = default_paths();

    let desktop_file = paths
        .find_map(|p| {
            let file = p.join(&format!("{}.desktop", app_id));
            if file.exists() { Some(file) } else { None }
        })
        .map(|df| -> Result<_> {
            let content = std::fs::read_to_string(&df)
                .with_context(|| format!("Failed to read desktop entry {df:?}"))?;

            let entry = DesktopEntry::from_str(&df, &content, None::<&[&str]>)?;

            Ok(entry
                .desktop_entry("Icon")
                .and_then(|icon_name| icon_finder::find_icon(icon_name.to_string(), 128, 1)))
        })
        .transpose()?
        .unwrap_or_else(|| icon_finder::find_icon("default-application".to_string(), 128, 1))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_ICON));

    println!("{:?}", desktop_file);

    Ok(desktop_file)
}

pub struct IconCache {
    inner: BTreeMap<String, Option<svg::Handle>>,
}

impl IconCache {
    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    pub fn get_icon(&mut self, app_id: &str) -> &Option<svg::Handle> {
        if !self.inner.contains_key(app_id) {
            let handle_option = client_icon(app_id)
                .map(svg::Handle::from_path)
                .map_err(|e| eprintln!("Failed to get icon for {app_id}: {e}"))
                .ok();

            self.inner.insert(app_id.to_string(), handle_option);
        }

        &self.inner[app_id]
    }
}
