use crate::{
    Message,
    config::{self, HydratedMouseBinds},
    constants::BAR_NAMESPACE,
};
use iced::{
    Element, Size,
    futures::Stream,
    mouse::ScrollDelta,
    widget::{MouseArea, container},
    window::settings::{
        Anchor, KeyboardInteractivity, Layer, LayerShellSettings,
        PlatformSpecific,
    },
};
use std::{
    path::{Path, PathBuf},
    pin::Pin,
};

use tracing::{Level, error};
use tracing_subscriber::{
    fmt::{self, writer::MakeWriterExt},
    registry::LookupSpan,
    reload,
};

pub type BoxStream<T> = Pin<Box<dyn Stream<Item = T> + Send>>;

type BoxedLayer<S> =
    Box<dyn tracing_subscriber::layer::Layer<S> + Send + Sync + 'static>;

const MAX_LOG_FILES: usize = 10;
pub fn init_tracing<S: tracing::Subscriber + for<'a> LookupSpan<'a>>(
    config_dir: &Path,
    handle: &reload::Handle<Option<BoxedLayer<S>>, S>,
) -> PathBuf {
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
                    .map_or(filename.as_str(), |(_head, digits)| digits);

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

    let boxed_layer: BoxedLayer<S> = Box::new(logfile_layer);

    if let Err(e) = handle.modify(|layer| *layer = Some(boxed_layer)) {
        tracing::error!("Failed to reload file logging layer: {}", e);
    }

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

#[profiling::function]
pub fn mouse_binds<'a>(
    element: impl Into<Element<'a, Message>>,
    binds: &'a HydratedMouseBinds,
    tooltip_id: Option<container::Id>,
) -> Element<'a, Message> {
    let mut mouse_area = MouseArea::new(element);

    if let Some(id) = tooltip_id {
        mouse_area = mouse_area
            .on_enter(Message::OpenTooltip(id.clone()))
            .on_exit(Message::CloseTooltip(id));
    }

    if let Some(left) = &binds.mouse_left {
        mouse_area = mouse_area.on_release(left.clone());
    }

    if let Some(double) = &binds.double_click {
        mouse_area = mouse_area.on_double_click(double.clone());
    }

    if let Some(right) = &binds.mouse_right {
        mouse_area = mouse_area.on_right_release(right.clone());
    }

    if let Some(middle) = &binds.mouse_middle {
        mouse_area = mouse_area.on_middle_release(middle.clone());
    }

    if let Some(ref scroll) = binds.scroll {
        mouse_area = mouse_area.on_scroll(|delta| {
            let (x, y) = match delta {
                ScrollDelta::Lines { x, y } | ScrollDelta::Pixels { x, y } => {
                    (x, y)
                }
            };

            if y > 0.0
                && let Some(up) = scroll.up.clone()
            {
                up
            } else if y < 0.0
                && let Some(down) = scroll.down.clone()
            {
                down
            } else if x < 0.0
                && let Some(right) = scroll.right.clone()
            {
                right
            } else if x > 0.0
                && let Some(left) = scroll.left.clone()
            {
                left
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
