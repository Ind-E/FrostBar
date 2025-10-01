use crate::{
    Message,
    config::{self, Config, Module, MouseBinds},
    constants::BAR_NAMESPACE,
    views::{
        BarAlignment, BarPosition, battery::BatteryView, cava::CavaView,
        label::LabelView, mpris::MprisView, niri::NiriView, time::TimeView,
    },
};
use iced::{
    Element, Size,
    futures::Stream,
    mouse::ScrollDelta,
    widget::MouseArea,
    window::settings::{
        Anchor, KeyboardInteractivity, Layer, LayerShellSettings,
        PlatformSpecific,
    },
};
use std::{
    path::{Path, PathBuf},
    pin::Pin,
};
use tracing::{Level, info};
use tracing_subscriber::{
    fmt::{self, writer::MakeWriterExt},
    prelude::__tracing_subscriber_SubscriberExt,
    util::SubscriberInitExt,
};

pub type BoxStream<T> = Pin<Box<dyn Stream<Item = T> + Send>>;

#[allow(clippy::too_many_arguments)]
#[profiling::function]
pub fn handle_module(
    module: Module,
    position: BarPosition,
    battery_views: &mut Vec<BatteryView>,
    time_views: &mut Vec<TimeView>,
    cava_views: &mut Vec<CavaView>,
    mpris_views: &mut Vec<MprisView>,
    niri_views: &mut Vec<NiriView>,
    label_views: &mut Vec<LabelView>,
) {
    match module {
        Module::Battery(config) => {
            battery_views.push(BatteryView::new(config, position));
        }
        Module::Time(config) => {
            time_views.push(TimeView::new(config, position));
        }
        Module::Cava(config) => {
            cava_views.push(CavaView::new(config, position));
        }
        Module::Mpris(config) => {
            mpris_views.push(MprisView::new(config, position));
        }
        Module::Niri(config) => {
            niri_views.push(NiriView::new(config, position));
        }
        Module::Label(config) => {
            label_views.push(LabelView::new(config, position));
        }
    }
}

#[profiling::function]
pub fn process_modules(
    config: &mut Config,
    battery_views: &mut Vec<BatteryView>,
    time_views: &mut Vec<TimeView>,
    cava_views: &mut Vec<CavaView>,
    mpris_views: &mut Vec<MprisView>,
    niri_views: &mut Vec<NiriView>,
    label_views: &mut Vec<LabelView>,
) {
    battery_views.clear();
    time_views.clear();
    cava_views.clear();
    mpris_views.clear();
    niri_views.clear();
    label_views.clear();

    let mut idx = 0;

    for module in config.start.modules.drain(..) {
        handle_module(
            module,
            BarPosition {
                idx,
                align: BarAlignment::Start,
            },
            battery_views,
            time_views,
            cava_views,
            mpris_views,
            niri_views,
            label_views,
        );
        idx += 1;
    }

    for module in config.middle.modules.drain(..) {
        handle_module(
            module,
            BarPosition {
                idx,
                align: BarAlignment::Middle,
            },
            battery_views,
            time_views,
            cava_views,
            mpris_views,
            niri_views,
            label_views,
        );
        idx += 1;
    }

    for module in config.end.modules.drain(..) {
        handle_module(
            module,
            BarPosition {
                idx,
                align: BarAlignment::End,
            },
            battery_views,
            time_views,
            cava_views,
            mpris_views,
            niri_views,
            label_views,
        );
        idx += 1;
    }
}

const LOG_FILE_PREFIX: &str = "frostbar.log";
const MAX_LOG_FILES: usize = 5;
pub fn init_tracing(config_dir: &Path) -> PathBuf {
    let debug = cfg!(debug_assertions);

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
                eprintln!("failed to remove old log file: {e}");
            }
            nlog += 1;
        }
        Err(e) => {
            eprintln!("failed to read log directory: {e}");
        }
    }

    let logfile = tracing_appender::rolling::never(
        &log_dir,
        format!("frostbar.log-{nlog}"),
    )
    .with_max_level(Level::INFO);

    let logfile_layer =
        fmt::layer().compact().with_writer(logfile).with_ansi(false);

    let stdout = std::io::stdout.with_max_level(if debug {
        Level::DEBUG
    } else {
        Level::INFO
    });

    let stdout_layer = fmt::layer().compact().with_writer(stdout);

    tracing_subscriber::registry()
        .with(logfile_layer)
        .with(stdout_layer)
        .init();

    log_dir.join(format!("frostbar.log-{nlog}"))
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
            Size::new(monitor_size.width, 0.0)
        }
        config::Anchor::Top | config::Anchor::Bottom => {
            Size::new(0.0, monitor_size.width)
        }
    };

    let anchor = Some(match layout.anchor {
        config::Anchor::Left => Anchor::LEFT | Anchor::TOP | Anchor::BOTTOM,
        config::Anchor::Right => Anchor::RIGHT | Anchor::TOP | Anchor::BOTTOM,
        config::Anchor::Top => Anchor::TOP | Anchor::LEFT | Anchor::RIGHT,
        config::Anchor::Bottom => Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
    });

    // top, right, bottom, left
    let margin = Some(match layout.anchor {
        config::Anchor::Left => (layout.gaps, 0, layout.gaps, layout.gaps),
        config::Anchor::Right => (layout.gaps, layout.gaps, layout.gaps, 0),
        config::Anchor::Top => (layout.gaps, layout.gaps, 0, layout.gaps),
        config::Anchor::Bottom => (0, layout.gaps, layout.gaps, layout.gaps),
    });

    // x, y, width, height
    let input_region = Some(match layout.anchor {
        config::Anchor::Left | config::Anchor::Right => {
            (0, 0, layout.width as i32, monitor_size.height as i32)
        }
        config::Anchor::Top | config::Anchor::Bottom => {
            (0, 0, monitor_size.width as i32, layout.width as i32)
        }
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
                layer: Some(Layer::Top),
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
pub fn maybe_mouse_binds<'a>(
    element: impl Into<Element<'a, Message>>,
    binds: &'a MouseBinds,
) -> Element<'a, Message> {
    if binds.mouse_left.is_none()
        && binds.mouse_right.is_none()
        && binds.mouse_middle.is_none()
        && binds.scroll_up.is_none()
        && binds.scroll_down.is_none()
    {
        element.into()
    } else {
        let mut mouse_area = MouseArea::new(element);
        if let Some(left) = &binds.mouse_left {
            mouse_area = mouse_area.on_release(process_command(left));
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
                    ScrollDelta::Lines { x, y }
                    | ScrollDelta::Pixels { x, y } => (x, y),
                };

                if (y > 0.0 || x < 0.0)
                    && let Some(scroll_up) = &binds.scroll_up
                {
                    return process_command(scroll_up);
                } else if let Some(scroll_down) = &binds.scroll_down {
                    return process_command(scroll_down);
                }
                unreachable!()
            });
        }

        mouse_area.into()
    }
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
            && args[0] == "-c"
        {
            let joined = args[1..].join(" ");
            write!(f, "{joined}")
        } else {
            write!(
                f,
                "{}",
                self.args
                    .as_ref()
                    .map(|v| format!("{} {}", self.command, v.join(" ")))
                    .unwrap_or_default()
            )
        }
    }
}
