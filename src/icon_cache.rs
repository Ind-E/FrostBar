use base64::Engine;
use color_thief::ColorFormat;
use freedesktop_icons::{default_theme_gtk, lookup};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use tracing::{debug, error};

use base64::engine::general_purpose;
use freedesktop_desktop_entry::{DesktopEntry, default_paths};
use iced::{
    Color,
    advanced::graphics::image::image_rs::{
        ImageBuffer, Rgb, load_from_memory, open,
    },
    widget::{
        image::{self},
        svg,
    },
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
            eprintln!(
                "Warning: Unrecognized or missing icon extension at path: {}",
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

pub struct MprisArtCache {
    inner: BTreeMap<String, (image::Handle, Option<Vec<Color>>)>,
}

#[profiling::all_functions]
impl MprisArtCache {
    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    pub fn get_art(
        &mut self,
        art_url: &str,
    ) -> Option<&(image::Handle, Option<Vec<Color>>)> {
        if self.inner.contains_key(art_url) {
            return Some(self.inner.get(art_url).unwrap());
        }

        let art = if let Some(url) =
            art_url.strip_prefix("data:image/jpeg;base64,")
        {
            let image_bytes = match general_purpose::STANDARD.decode(url) {
                Ok(bytes) => bytes,
                Err(e) => {
                    eprintln!("icon_cache get_art error: {e}");
                    return None;
                }
            };
            let gradient = load_from_memory(&image_bytes)
                .ok()
                .and_then(|img| extract_gradient(&img.to_rgb8(), 12));
            let handle = image::Handle::from_bytes(image_bytes);
            (handle, gradient)
        } else if let Some(url) = art_url.strip_prefix("file://") {
            let handle = image::Handle::from_path(url);
            let gradient = open(url)
                .ok()
                .and_then(|img| extract_gradient(&img.to_rgb8(), 12));
            (handle, gradient)
        } else if art_url.starts_with("https://")
            || art_url.starts_with("http://")
        {
            let response = match reqwest::blocking::get(art_url) {
                Ok(res) => res,
                Err(e) => {
                    error!("Failed to fetch album art: {e}");
                    return None;
                }
            };
            let image_bytes = match response.bytes() {
                Ok(bytes) => bytes,
                Err(e) => {
                    error!(
                        "Failed to get bytes of album art from {art_url}: {e}"
                    );
                    return None;
                }
            };

            let gradient = load_from_memory(&image_bytes)
                .ok()
                .and_then(|img| extract_gradient(&img.to_rgb8(), 12));
            let handle = image::Handle::from_bytes(image_bytes);
            (handle, gradient)
        } else {
            return None;
        };

        self.inner.insert(art_url.to_string(), art);
        Some(self.inner.get(art_url).unwrap())
    }
}

fn lerp_color(c1: Color, c2: Color, factor: f32) -> Color {
    let r = c1.r * (1.0 - factor) + c2.r * factor;
    let g = c1.g * (1.0 - factor) + c2.g * factor;
    let b = c1.b * (1.0 - factor) + c2.b * factor;
    Color::from_rgba(r, g, b, 1.0)
}

#[profiling::function]
fn generate_gradient(
    palette: Vec<color_thief::Color>,
    steps: usize,
) -> Option<Vec<Color>> {
    if palette.is_empty() {
        return None;
    }

    let iced_palette: Vec<Color> = palette
        .into_iter()
        .map(|c| Color::from_rgb8(c.r, c.g, c.b))
        .collect();

    if iced_palette.len() == 1 {
        return Some(vec![iced_palette[0]; steps]);
    }

    let mut gradient = Vec::with_capacity(steps);
    let segments = (iced_palette.len() - 1) as f32;

    for i in 0..steps {
        let progress = if steps == 1 {
            0.0
        } else {
            i as f32 / (steps - 1) as f32
        };
        let position = progress * segments;

        let start_index = position.floor() as usize;
        let end_index = (start_index + 1).min(iced_palette.len() - 1);

        let factor = position.fract();

        let start_color = iced_palette[start_index];
        let end_color = iced_palette[end_index];

        gradient.push(lerp_color(start_color, end_color, factor));
    }

    Some(gradient)
}

#[profiling::function]
fn extract_gradient(
    buffer: &ImageBuffer<Rgb<u8>, Vec<u8>>,
    bars: usize,
) -> Option<Vec<Color>> {
    match color_thief::get_palette(buffer.as_raw(), ColorFormat::Rgb, 10, 3) {
        Ok(palette) => generate_gradient(palette, bars * 2),
        Err(_) => None,
    }
}
