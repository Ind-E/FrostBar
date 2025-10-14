use crate::{
    Message,
    config::{self, MouseBinds},
    constants::BAR_NAMESPACE,
};
use iced::{
    Color, Element, Size,
    advanced::graphics::image::image_rs,
    futures::Stream,
    mouse::ScrollDelta,
    widget::{MouseArea, image},
    window::settings::{
        Anchor, KeyboardInteractivity, Layer, LayerShellSettings,
        PlatformSpecific,
    },
};
use std::{
    path::{Path, PathBuf},
    pin::Pin,
};

use base64::Engine;
use tracing::{Dispatch, Level, error};
use tracing_subscriber::{
    EnvFilter,
    fmt::{self, writer::MakeWriterExt},
    prelude::__tracing_subscriber_SubscriberExt,
    util::SubscriberInitExt,
};

pub type BoxStream<T> = Pin<Box<dyn Stream<Item = T> + Send>>;

const MAX_LOG_FILES: usize = 10;
pub fn init_tracing(config_dir: &Path) -> PathBuf {
    let temp_sub = tracing_subscriber::fmt()
        .compact()
        .with_writer(std::io::stderr)
        .with_max_level(Level::DEBUG)
        .finish();

    let temp_dispatch = Dispatch::new(temp_sub);

    let (filter, logfile_layer, stderr_layer, logfile_path) =
        tracing::dispatcher::with_default(&temp_dispatch, || {
            let debug = cfg!(debug_assertions);

            let filter =
                EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                    if debug {
                        EnvFilter::new("info,frostbar=debug")
                    } else {
                        EnvFilter::new("error,frostbar=info")
                    }
                });

            let log_dir = config_dir.join("logs/");
            let mut nlog = 0;
            let mut min_log = u64::MAX;

            match read_log_dir(&log_dir) {
                Ok(log_files) => {
                    let mut num_files = 0;
                    for filename in log_files {
                        num_files += 1;
                        let trailing = filename
                            .rsplit_once(|c: char| !c.is_ascii_digit())
                            .map_or(filename.as_str(), |(_head, digits)| {
                                digits
                            });

                        if let Ok(trailing_digits) = trailing.parse::<u64>() {
                            if trailing_digits > nlog {
                                nlog = trailing_digits;
                            }
                            if trailing_digits < min_log {
                                min_log = trailing_digits;
                            }
                        }
                    }
                    if num_files >= MAX_LOG_FILES
                        && let Err(e) = std::fs::remove_file(
                            log_dir.join(format!("frostbar.log-{min_log}")),
                        )
                    {
                        error!("failed to remove old log file: {e}");
                    }
                    nlog += 1;
                }
                Err(e) => {
                    error!("failed to read log directory: {e}");
                }
            }

            let logfile_path = log_dir.join(format!("frostbar.log-{nlog}"));

            let logfile = tracing_appender::rolling::never(
                &log_dir,
                logfile_path.file_name().unwrap(),
            )
            .with_max_level(Level::INFO);

            let logfile_layer =
                fmt::layer().compact().with_writer(logfile).with_ansi(false);

            let stderr_layer =
                fmt::layer().compact().with_writer(std::io::stderr);

            (filter, logfile_layer, stderr_layer, logfile_path)
        });

    tracing_subscriber::registry()
        .with(filter)
        .with(logfile_layer)
        .with(stderr_layer)
        .init();

    logfile_path
}

fn read_log_dir(path: &Path) -> std::io::Result<Vec<String>> {
    let mut filenames = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file()
            && let Some(name) = path.file_name().and_then(|n| n.to_str())
        {
            filenames.push(name.to_string());
        }
    }
    Ok(filenames)
}

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

#[profiling::function]
pub fn mouse_binds<'a>(
    element: impl Into<Element<'a, Message>>,
    // tooltip_content: impl Into<Element<'a, Message>>,
    // tooltip: Arc<RwLock<Element<'a, Message>>>,
    binds: &'a MouseBinds,
) -> Element<'a, Message> {
    let mut mouse_area = MouseArea::new(element);
    if let Some(left) = &binds.mouse_left {
        mouse_area = mouse_area.on_release(process_command(left));
    }

    if let Some(double) = &binds.double_click {
        mouse_area = mouse_area.on_double_click(process_command(double));
    }

    if let Some(right) = &binds.mouse_right {
        mouse_area = mouse_area.on_right_release(process_command(right));
    }

    if let Some(middle) = &binds.mouse_middle {
        mouse_area = mouse_area.on_middle_release(process_command(middle));
    }

    if binds.scroll_up.is_some() || binds.scroll_down.is_some() {
        mouse_area = mouse_area.on_scroll(|delta| {
            let (x, y) = match delta {
                ScrollDelta::Lines { x, y } | ScrollDelta::Pixels { x, y } => {
                    (x, y)
                }
            };

            if y > 0.0
                && let Some(scroll_up) = &binds.scroll_up
            {
                process_command(scroll_up)
            } else if y < 0.0
                && let Some(scroll_down) = &binds.scroll_down
            {
                process_command(scroll_down)
            } else if x < 0.0
                && let Some(scroll_right) = &binds.scroll_right
            {
                process_command(scroll_right)
            } else if x > 0.0
                && let Some(scroll_left) = &binds.scroll_left
            {
                process_command(scroll_left)
            } else {
                Message::NoOp
            }
        });
    }

    mouse_area.into()
}

#[profiling::function]
pub fn process_command(cmd: &config::Command) -> Message {
    if cmd.args.is_empty() {
        Message::NoOp
    } else if let Some(sh) = cmd.sh
        && sh
    {
        Message::Command(CommandSpec {
            command: String::from("sh"),
            args: Some(vec![String::from("-c"), cmd.args[0].clone()]),
        })
    } else {
        Message::Command(CommandSpec {
            command: cmd.args[0].clone(),
            args: cmd.args.get(1..).map(<[String]>::to_vec),
        })
    }
}

#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub command: String,
    pub args: Option<Vec<String>>,
}

impl std::fmt::Display for CommandSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(args) = self.args.as_ref()
            && !args.is_empty()
        {
            if args[0] == "-c" {
                write!(f, "{}", args[1..].join(" "))
            } else {
                write!(f, "{} {}", self.command, args.join(" "))
            }
        } else {
            write!(f, "{}", self.command)
        }
    }
}

pub fn get_art(art_url: &str) -> Option<(image::Handle, Option<Vec<Color>>)> {
    if let Some(url) = art_url.strip_prefix("data:image/jpeg;base64,") {
        let image_bytes =
            match base64::engine::general_purpose::STANDARD.decode(url) {
                Ok(bytes) => bytes,
                Err(e) => {
                    error!("get_art error: {e}");
                    return None;
                }
            };
        let gradient = image_rs::load_from_memory(&image_bytes)
            .ok()
            .and_then(|img| extract_gradient(&img.to_rgb8(), 12));
        let handle = image::Handle::from_bytes(image_bytes);
        Some((handle, gradient))
    } else if let Some(url) = art_url.strip_prefix("file://") {
        let handle = image::Handle::from_path(url);
        let gradient = image_rs::open(url)
            .ok()
            .and_then(|img| extract_gradient(&img.to_rgb8(), 12));
        Some((handle, gradient))
    } else if art_url.starts_with("https://") || art_url.starts_with("http://")
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
                error!("Failed to get bytes of album art from {art_url}: {e}");
                return None;
            }
        };

        let gradient = image_rs::load_from_memory(&image_bytes)
            .ok()
            .and_then(|img| extract_gradient(&img.to_rgb8(), 12));
        let handle = image::Handle::from_bytes(image_bytes);
        Some((handle, gradient))
    } else {
        None
    }
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

fn lerp_color(c1: Color, c2: Color, factor: f32) -> Color {
    let r = c1.r * (1.0 - factor) + c2.r * factor;
    let g = c1.g * (1.0 - factor) + c2.g * factor;
    let b = c1.b * (1.0 - factor) + c2.b * factor;
    Color::from_rgba(r, g, b, 1.0)
}

#[profiling::function]
fn extract_gradient(
    buffer: &image_rs::ImageBuffer<image_rs::Rgb<u8>, Vec<u8>>,
    bars: usize,
) -> Option<Vec<Color>> {
    match color_thief::get_palette(
        buffer.as_raw(),
        color_thief::ColorFormat::Rgb,
        10,
        3,
    ) {
        Ok(palette) => generate_gradient(palette, bars * 2),
        Err(_) => None,
    }
}
