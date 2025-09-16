use crate::{
    Message,
    config::{self, Config, Module, MouseInteraction},
    constants::BAR_NAMESPACE,
    views::{
        BarAlignment, BarPosition, battery::BatteryView, cava::CavaView,
        label::LabelView, mpris::MprisView, niri::NiriView, time::TimeView,
    },
};
use iced::{
    Element, Size,
    mouse::ScrollDelta,
    widget::MouseArea,
    window::settings::{
        Anchor, KeyboardInteractivity, Layer, LayerShellSettings, PlatformSpecific,
    },
};
use std::fmt;
use tracing_subscriber::EnvFilter;

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

pub fn open_window(config: &Config) -> (iced::window::Id, iced::Task<Message>) {
    let (id, open_task) = iced::window::open(iced::window::Settings {
        size: Size::new(config.layout.width as f32, 0.0),
        decorations: false,
        resizable: false,
        minimizable: false,
        transparent: true,
        platform_specific: PlatformSpecific {
            layer_shell: LayerShellSettings {
                layer: Some(Layer::Top),
                anchor: Some(Anchor::LEFT | Anchor::TOP | Anchor::BOTTOM | Anchor::RIGHT),
                exclusive_zone: Some(config.layout.width as i32),
                margin: Some((
                    config.layout.gaps,
                    config.layout.gaps,
                    config.layout.gaps,
                    config.layout.gaps,
                )),
                input_region: Some((0, 0, config.layout.width as i32, 1200)),
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

pub fn maybe_mouse_interaction<'a>(
    element: impl Into<Element<'a, Message>>,
    interaction: &'a MouseInteraction,
) -> Element<'a, Message> {
    if interaction.left_mouse.is_none()
        && interaction.right_mouse.is_none()
        && interaction.middle_mouse.is_none()
    {
        return element.into();
    } else {
        let mut mouse_area = MouseArea::new(element);
        if let Some(left) = &interaction.left_mouse {
            mouse_area = mouse_area.on_release(process_command(left));
        }

        if let Some(right) = &interaction.right_mouse {
            mouse_area = mouse_area.on_right_release(process_command(right));
        }

        if let Some(middle) = &interaction.middle_mouse {
            mouse_area = mouse_area.on_middle_release(process_command(middle));
        }

        if interaction.scroll_up.is_some() || interaction.scroll_down.is_some() {
            mouse_area = mouse_area.on_scroll(|delta| {
                let (x, y) = match delta {
                    ScrollDelta::Lines { x, y } | ScrollDelta::Pixels { x, y } => (x, y),
                };

                if y > 0.0 || x < 0.0 {
                    if let Some(scroll_up) = &interaction.scroll_up {
                        return process_command(scroll_up);
                    }
                } else {
                    if let Some(scroll_down) = &interaction.scroll_down {
                        return process_command(scroll_down);
                    }
                };
                unreachable!()
            })
        }

        mouse_area.into()
    }
}

pub fn process_command(cmd: &config::Command) -> Message {
    if let Some(sh) = &cmd.sh {
        Message::Command(CommandSpec {
            command: String::from("sh"),
            args: Some(vec![String::from("-c"), sh.to_string()]),
        })
    } else if let Some(args) = &cmd.command
        && args.len() > 0
    {
        Message::Command(CommandSpec {
            command: String::from(args[0].clone()),
            args: args.get(1..).and_then(|v| Some(v.to_vec())),
        })
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
            return write!(f, "{}", joined);
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
