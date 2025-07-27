use base64::Engine;
use color_thief::ColorFormat;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

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
use xdgkit::icon_finder;

use crate::config::CAVA_BARS;

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

#[derive(Debug)]
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
}

pub struct MprisArtCache {
    inner: BTreeMap<String, (Option<image::Handle>, Option<Vec<Color>>)>,
}

impl MprisArtCache {
    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    pub fn get_art(
        &mut self,
        art_url: &str,
    ) -> &(Option<image::Handle>, Option<Vec<Color>>) {
        self.inner.entry(art_url.to_string()).or_insert_with(|| {
            if let Some(url) = art_url.strip_prefix("data:image/jpeg;base64,") {
                let image_bytes = match general_purpose::STANDARD.decode(url) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        eprintln!("icon_cache get_art error: {e}");
                        return (None, None);
                    }
                };
                let handle = image::Handle::from_bytes(image_bytes.clone());
                let gradient = load_from_memory(&image_bytes)
                    .ok()
                    .and_then(|img| extract_gradient(&img.to_rgb8()));
                (Some(handle), gradient)
            } else if let Some(url) = art_url.strip_prefix("file://") {
                let handle = image::Handle::from_path(url);
                let gradient = open(url)
                    .ok()
                    .and_then(|img| extract_gradient(&img.to_rgb8()));
                (Some(handle), gradient)
            } else {
                (None, None)
            }
        })
    }
}

fn lerp_color(c1: Color, c2: Color, factor: f32) -> Color {
    let r = c1.r * (1.0 - factor) + c2.r * factor;
    let g = c1.g * (1.0 - factor) + c2.g * factor;
    let b = c1.b * (1.0 - factor) + c2.b * factor;
    Color::new(r, g, b, 1.0)
}

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

fn extract_gradient(
    buffer: &ImageBuffer<Rgb<u8>, Vec<u8>>,
) -> Option<Vec<Color>> {
    match color_thief::get_palette(buffer.as_raw(), ColorFormat::Rgb, 10, 3) {
        Ok(palette) => generate_gradient(palette, CAVA_BARS * 2),
        Err(_) => None,
    }
}
