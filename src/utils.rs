use crate::{
    Message,
    config::{self, Bind, Config, Module, MouseTrigger},
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
        Anchor, KeyboardInteractivity, Layer, LayerShellSettings, PlatformSpecific,
    },
};
use std::{fmt, pin::Pin};
use tracing_subscriber::EnvFilter;

pub type BoxStream<T> = Pin<Box<dyn Stream<Item = T> + Send>>;

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

pub fn init_tracing() {
    let default_level = if cfg!(debug_assertions) {
        "debug"
    } else {
        "info"
    };

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(default_level)),
        )
        .with_timer(tracing_subscriber::fmt::time::ChronoLocal::new(
            "%H:%M:%S".to_string(),
        ))
        .with_line_number(true)
        .init();
}

pub fn open_window(layout: &config::Layout) -> (iced::window::Id, iced::Task<Message>) {
    let (id, open_task) = iced::window::open(iced::window::Settings {
        size: Size::new(layout.width as f32, 0.0),
        decorations: false,
        resizable: false,
        minimizable: false,
        transparent: true,
        platform_specific: PlatformSpecific {
            layer_shell: LayerShellSettings {
                layer: Some(Layer::Top),
                anchor: Some(Anchor::LEFT | Anchor::TOP | Anchor::BOTTOM | Anchor::RIGHT),
                exclusive_zone: Some(layout.width as i32 + layout.gaps),
                // no right gaps because right edge of the layer surface is actually the right edge
                // of the screen. Instead, increase the exclusive zone to emulate gaps
                margin: Some((layout.gaps, 0, layout.gaps, layout.gaps)),
                input_region: Some((0, 0, layout.width as i32, 1200)),
                keyboard_interactivity: Some(KeyboardInteractivity::None),
                namespace: Some(String::from(BAR_NAMESPACE)),
                ..Default::default()
            },
            ..Default::default()
        },
        exit_on_close_request: false,
        ..Default::default()
    });

    (id, open_task.map(Message::OpenWindow))
}

pub fn maybe_mouse_binds<'a>(
    element: impl Into<Element<'a, Message>>,
    binds: &'a Vec<Bind>,
) -> Element<'a, Message> {
    let mut mouse_area = MouseArea::new(element);

    let mut scroll_binds = (None, None);

    for bind in binds {
        match bind.trigger {
            MouseTrigger::MouseLeft => {
                mouse_area = mouse_area.on_release(process_command(&bind.action));
            }

            MouseTrigger::MouseRight => {
                mouse_area = mouse_area.on_right_release(process_command(&bind.action));
            }

            MouseTrigger::MouseMiddle => {
                mouse_area = mouse_area.on_middle_release(process_command(&bind.action));
            }
            MouseTrigger::ScrollUp => {
                scroll_binds.0 = Some(&bind.action);
            }

            MouseTrigger::ScrollDown => {
                scroll_binds.1 = Some(&bind.action);
            }
        }
    }

    if scroll_binds.0.is_some() || scroll_binds.1.is_some() {
        mouse_area = mouse_area.on_scroll(move |delta| {
            let (x, y) = match delta {
                ScrollDelta::Lines { x, y } | ScrollDelta::Pixels { x, y } => (x, y),
            };

            if (y > 0.0 || x < 0.0)
                && let Some(scroll_up) = scroll_binds.0
            {
                return process_command(&scroll_up);
            } else if let Some(scroll_down) = scroll_binds.1 {
                return process_command(&scroll_down);
            }
            unreachable!()
        });
    }

    mouse_area.into()
}

pub fn process_command(cmd: &config::Command) -> Message {
    if !cmd.args.is_empty() {
        if let Some(sh) = cmd.sh
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
    } else {
        Message::NoOp
    }
}

#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub command: String,
    pub args: Option<Vec<String>>,
}

impl fmt::Display for CommandSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
