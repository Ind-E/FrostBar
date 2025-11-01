use std::{
    ffi::OsStr,
    fs::{self, File},
    io::Write,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
};

use directories::ProjectDirs;
use iced::{Background, Color, border, color, widget::container};

use knus::{
    Decode, DecodeScalar, ast::Literal, decode::Kind, errors::DecodeError,
};
use miette::{Context, IntoDiagnostic};
use notify_rust::Notification;
use rustc_hash::FxHashMap;
use tracing::{debug, error, info};

use crate::{
    Message,
    constants::BAR_NAMESPACE,
    file_watcher::ConfigPath,
    utils::CommandSpec,
    views::{BarAlignment, BarPosition},
};

#[derive(knus::Decode, Default, Debug)]
pub struct Config {
    #[knus(child, default)]
    pub layout: Layout,
    #[knus(child, default)]
    pub style: Style,
    #[knus(child, default)]
    pub start: Start,
    #[knus(child, default)]
    pub middle: Middle,
    #[knus(child, default)]
    pub end: End,
}

pub struct HydratedConfig {
    pub layout: Layout,
    pub style: HydratedStyle,
    pub modules: HydratedConfigModules,
}

impl Config {
    pub fn hydrate(self, colors: &ColorVars) -> HydratedConfig {
        HydratedConfig {
            layout: self.layout,
            style: self.style.hydrate(colors),
            modules: hydrate_modules(
                (self.start, self.middle, self.end),
                colors,
            ),
        }
    }
}

fn hydrate_modules(
    value: (Start, Middle, End),
    colors: &ColorVars,
) -> HydratedConfigModules {
    let mut modules = Vec::new();

    let mut process_section =
        |mut module_configs: Vec<ConfigModule>, align: BarAlignment| {
            for (idx, module_config) in module_configs.drain(..).enumerate() {
                let position = BarPosition { idx, align };
                match module_config {
                    ConfigModule::Battery(c) => {
                        modules.push((c.hydrate(colors), position))
                    }
                    ConfigModule::Cava(c) => {
                        modules.push((c.hydrate(colors), position));
                    }
                    ConfigModule::Mpris(c) => {
                        modules.push((c.hydrate(colors), position));
                    }
                    ConfigModule::Time(c) => {
                        modules.push((c.hydrate(colors), position));
                    }
                    ConfigModule::Label(c) => {
                        modules.push((c.hydrate(colors), position));
                    }
                    ConfigModule::Niri(c) => {
                        modules.push((c.hydrate(colors), position));
                    }
                }
            }
        };

    process_section(value.0.modules, BarAlignment::Start);
    process_section(value.1.modules, BarAlignment::Middle);
    process_section(value.2.modules, BarAlignment::End);

    HydratedConfigModules { inner: modules }
}

#[derive(knus::Decode, Debug, Default)]
pub struct Start {
    #[knus(children, default)]
    pub modules: Vec<ConfigModule>,
}

#[derive(knus::Decode, Debug, Default)]
pub struct Middle {
    #[knus(children, default)]
    pub modules: Vec<ConfigModule>,
}

#[derive(knus::Decode, Debug, Default)]
pub struct End {
    #[knus(children, default)]
    pub modules: Vec<ConfigModule>,
}

#[derive(knus::Decode, Debug, Clone, PartialEq)]
pub struct Layout {
    #[knus(child, unwrap(argument), default = 42)]
    pub width: u32,
    #[knus(child, unwrap(argument), default = 0)]
    pub gaps: i32,
    #[knus(child, unwrap(argument), default = Self::default().anchor)]
    pub anchor: Anchor,
    #[knus(child, unwrap(argument), default = Self::default().layer)]
    pub layer: Layer,
}

#[derive(knus::DecodeScalar, Debug, Clone, Copy, PartialEq)]
pub enum Anchor {
    Left,
    Right,
    Top,
    Bottom,
}

impl Anchor {
    pub fn vertical(self) -> bool {
        matches!(self, Anchor::Left | Anchor::Right)
    }

    pub fn top_left(self) -> bool {
        matches!(self, Anchor::Left | Anchor::Top)
    }
}

#[derive(knus::DecodeScalar, Debug, Clone, Copy, PartialEq)]
pub enum Layer {
    Background,
    Bottom,
    Top,
    Overlay,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            width: 42,
            gaps: 3,
            anchor: Anchor::Left,
            layer: Layer::Top,
        }
    }
}

#[derive(knus::Decode, Debug, Clone)]
pub struct Style {
    #[knus(child, default = Self::default().border_radius)]
    pub border_radius: ConfigRadius,
    #[knus(child, unwrap(argument), default = Self::default().background)]
    pub background: ConfigColor,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            border_radius: ConfigRadius::All(0.0),
            background: Color::from_rgb(0.0, 0.0, 0.0).into(),
        }
    }
}

pub struct HydratedStyle {
    pub border_radius: border::Radius,
    pub background: Color,
}

impl Style {
    fn hydrate(self, colors: &ColorVars) -> HydratedStyle {
        HydratedStyle {
            border_radius: self.border_radius.into(),
            background: self.background.resolve(colors),
        }
    }
}

#[derive(knus::Decode, Debug)]
struct ColorVariable {
    #[knus(node_name)]
    pub name: String,
    #[knus(argument)]
    pub color: ConfigColor,
}

#[derive(Debug, Default)]
pub struct ColorVars {
    vars: FxHashMap<String, Color>,
}

impl<S> knus::DecodeChildren<S> for ColorVars
where
    S: knus::traits::ErrorSpan,
{
    fn decode_children(
        nodes: &[knus::ast::SpannedNode<S>],
        ctx: &mut knus::decode::Context<S>,
    ) -> Result<Self, DecodeError<S>> {
        let mut vars = FxHashMap::default();
        for node in nodes {
            let var = ColorVariable::decode_node(node, ctx)?;
            vars.insert(var.name, var.color.parse());
        }

        Ok(Self { vars })
    }
}

#[derive(knus::Decode, Debug)]
pub enum ConfigModule {
    Cava(Cava),
    Battery(Battery),
    Time(Time),
    Mpris(Mpris),
    Niri(Niri),
    Label(Label),
}

pub enum HydratedConfigModule {
    Cava(HydratedCava),
    Battery(HydratedBattery),
    Time(HydratedTime),
    Mpris(HydratedMpris),
    Niri(HydratedNiri),
    Label(HydratedLabel),
}

pub struct HydratedConfigModules {
    inner: Vec<(HydratedConfigModule, BarPosition)>,
}

impl std::ops::Deref for HydratedConfigModules {
    type Target = Vec<(HydratedConfigModule, BarPosition)>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::ops::DerefMut for HydratedConfigModules {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[derive(knus::Decode, Debug)]
pub struct Cava {
    #[knus(child, unwrap(argument), default = Self::default().spacing)]
    pub spacing: f32,

    #[knus(child, unwrap(argument), default = Self::default().color)]
    pub color: ConfigColor,

    #[knus(child, unwrap(argument), default = Self::default().dynamic_color)]
    pub dynamic_color: bool,

    #[knus(flatten(child), default)]
    pub binds: MouseBinds,

    #[knus(child, default)]
    pub style: ContainerStyle,
}

impl Default for Cava {
    fn default() -> Self {
        Self {
            spacing: 0.1,
            dynamic_color: true,
            color: Color::WHITE.into(),
            binds: MouseBinds::default(),
            style: ContainerStyle::default(),
        }
    }
}

impl Cava {
    fn hydrate(self, colors: &ColorVars) -> HydratedConfigModule {
        let cava = HydratedCava {
            spacing: self.spacing,
            color: self.color.resolve(colors),
            dynamic_color: self.dynamic_color,
            binds: self.binds.hydrate(),
            style: self.style.hydrate(colors),
        };
        HydratedConfigModule::Cava(cava)
    }
}

pub struct HydratedCava {
    pub spacing: f32,
    pub color: Color,
    pub dynamic_color: bool,
    pub binds: HydratedMouseBinds,
    pub style: HydratedContainerStyle,
}

#[derive(knus::Decode, Debug)]
pub struct Battery {
    #[knus(child, unwrap(argument), default = Self::default().icon_size)]
    pub icon_size: u32,

    #[knus(child, unwrap(argument), default = Self::default().charging_color)]
    pub charging_color: ConfigColor,

    #[knus(child, default)]
    pub style: ContainerStyle,

    #[knus(flatten(child), default)]
    pub binds: MouseBinds,
}

impl Default for Battery {
    fn default() -> Self {
        Self {
            icon_size: 22,
            charging_color: color!(0x73F5AB).into(),
            style: ContainerStyle::default(),
            binds: MouseBinds::default(),
        }
    }
}

impl Battery {
    fn hydrate(self, colors: &ColorVars) -> HydratedConfigModule {
        let battery = HydratedBattery {
            icon_size: self.icon_size,
            charging_color: self.charging_color.resolve(colors),
            style: self.style.hydrate(colors),
            binds: self.binds.hydrate(),
        };

        HydratedConfigModule::Battery(battery)
    }
}

pub struct HydratedBattery {
    pub icon_size: u32,
    pub charging_color: Color,
    pub style: HydratedContainerStyle,
    pub binds: HydratedMouseBinds,
}

#[derive(knus::Decode, Debug)]
pub struct Time {
    #[knus(child, unwrap(argument), default = "%I\n%M".to_string())]
    pub format: String,

    #[knus(child, unwrap(argument), default = "%a %b %-d\n%-m/%-d/%y".to_string())]
    pub tooltip_format: String,

    #[knus(flatten(child), default)]
    pub binds: MouseBinds,

    #[knus(child, default)]
    pub style: ContainerStyle,
}

impl Time {
    fn hydrate(self, colors: &ColorVars) -> HydratedConfigModule {
        let time = HydratedTime {
            format: self.format,
            tooltip_format: self.tooltip_format,
            binds: self.binds.hydrate(),
            style: self.style.hydrate(colors),
        };

        HydratedConfigModule::Time(time)
    }
}

pub struct HydratedTime {
    pub format: String,
    pub tooltip_format: String,
    pub binds: HydratedMouseBinds,
    pub style: HydratedContainerStyle,
}

#[derive(knus::Decode, Debug, Clone)]
pub struct Mpris {
    #[knus(child, unwrap(argument), default = "󰝚".to_string())]
    pub placeholder: String,

    #[knus(flatten(child), default)]
    pub binds: MouseBindsForMpris,

    #[knus(child, default)]
    pub placeholder_style: ContainerStyle,
}

impl Mpris {
    fn hydrate(self, colors: &ColorVars) -> HydratedConfigModule {
        let mpris = HydratedMpris {
            placeholder: self.placeholder,
            binds: self.binds,
            placeholder_style: self.placeholder_style.hydrate(colors),
        };

        HydratedConfigModule::Mpris(mpris)
    }
}

pub struct HydratedMpris {
    pub placeholder: String,
    pub binds: MouseBindsForMpris,
    pub placeholder_style: HydratedContainerStyle,
}

#[derive(knus::Decode, Debug, Clone)]
pub struct Niri {
    #[knus(child, unwrap(argument), default = 10)]
    pub spacing: u32,

    #[knus(child, unwrap(argument), default = 0)]
    pub workspace_offset: i8,

    #[knus(child, default)]
    pub style: ContainerStyle,

    #[knus(child, default)]
    pub window_focused_style: ContainerStyle,

    #[knus(child, default)]
    pub window_style: ContainerStyle,

    #[knus(child, default)]
    pub workspace_active_style: ContainerStyle,

    #[knus(child, default)]
    pub workspace_hovered_style: ContainerStyle,

    #[knus(child, default)]
    pub workspace_style: ContainerStyle,
}

impl Niri {
    fn hydrate(self, colors: &ColorVars) -> HydratedConfigModule {
        let default_style = self.workspace_style.hydrate(colors);

        let mut hovered_style = default_style.clone();
        if let Some(text_color) = &self
            .workspace_hovered_style
            .text_color
            .map(|c| c.resolve(colors))
        {
            hovered_style.text_color = Some(*text_color);
        }
        if let Some(background) = &self
            .workspace_hovered_style
            .background
            .map(|c| c.resolve(colors))
        {
            hovered_style.background = Some(Background::Color(*background))
        }
        if let Some(border) = &self.workspace_hovered_style.border {
            let mut hovered_border = iced::Border::default();
            if let Some(color) =
                border.color.as_ref().map(|c| c.resolve(colors))
            {
                hovered_border.color = color;
            }
            if let Some(width) = border.width {
                hovered_border.width = width;
            }
            if let Some(radius) = &border.radius {
                hovered_border.radius = radius.clone().into();
            }
            hovered_style.border = hovered_border;
        }

        let mut active_style = default_style.clone();
        let mut active_hovered_style = hovered_style.clone();
        if let Some(text_color) = &self
            .workspace_active_style
            .text_color
            .map(|c| c.resolve(colors))
        {
            active_style.text_color = Some(*text_color);
            active_hovered_style.text_color = Some(*text_color);
        }
        if let Some(background) = &self
            .workspace_active_style
            .background
            .map(|c| c.resolve(colors))
        {
            active_style.background = Some(Background::Color(*background));
            active_hovered_style.background =
                Some(Background::Color(*background));
        }

        if let Some(border) = &self.workspace_active_style.border {
            let mut active_border = iced::Border::default();
            let mut active_hovered_border = iced::Border::default();
            if let Some(color) =
                border.color.as_ref().map(|c| c.resolve(colors))
            {
                active_border.color = color;
                active_hovered_border.color = color;
            }
            if let Some(width) = border.width {
                active_border.width = width;
                active_hovered_border.width = width;
            }
            if let Some(radius) = &border.radius {
                active_border.radius = radius.clone().into();
                active_hovered_border.radius = radius.clone().into();
            }
            active_style.border = active_border;
            active_hovered_style.border = active_hovered_border;
        }

        let workspace_active_hovered_style_merged = active_hovered_style;
        let workspace_active_style_merged = active_style;
        let workspace_hovered_style_merged = hovered_style;
        let window_focused_style_merged = HydratedContainerStyle::default();

        let niri = HydratedNiri {
            spacing: self.spacing,
            workspace_offset: self.workspace_offset,
            style: self.style.hydrate(colors),
            workspace_active_hovered_style_merged,
            workspace_active_style_merged,
            workspace_hovered_style_merged,
            workspace_default_style: default_style,
            window_focused_style_merged,
            window_default_style: self.window_style.hydrate(colors),
        };

        HydratedConfigModule::Niri(niri)
    }
}

pub struct HydratedNiri {
    pub spacing: u32,
    pub workspace_offset: i8,
    pub style: HydratedContainerStyle,
    pub workspace_active_hovered_style_merged: HydratedContainerStyle,
    pub workspace_active_style_merged: HydratedContainerStyle,
    pub workspace_hovered_style_merged: HydratedContainerStyle,
    pub workspace_default_style: HydratedContainerStyle,
    pub window_focused_style_merged: HydratedContainerStyle,
    pub window_default_style: HydratedContainerStyle,
}

#[derive(knus::Decode, Debug)]
pub struct Label {
    #[knus(child, unwrap(argument), default = String::new())]
    pub text: String,

    #[knus(child, unwrap(argument), default = 18)]
    pub size: u32,

    #[knus(child, unwrap(argument), default = None)]
    pub tooltip: Option<String>,

    #[knus(flatten(child), default)]
    pub binds: MouseBinds,

    #[knus(child, default)]
    pub style: ContainerStyle,
}

impl Label {
    fn hydrate(self, colors: &ColorVars) -> HydratedConfigModule {
        let label = HydratedLabel {
            text: self.text,
            size: self.size,
            tooltip: self.tooltip,
            binds: self.binds.hydrate(),
            style: self.style.hydrate(colors),
        };

        HydratedConfigModule::Label(label)
    }
}

pub struct HydratedLabel {
    pub text: String,
    pub size: u32,
    pub tooltip: Option<String>,
    pub binds: HydratedMouseBinds,
    pub style: HydratedContainerStyle,
}

#[derive(knus::Decode, Debug, Clone, Default)]
pub struct MouseBinds {
    #[knus(child)]
    pub mouse_left: Option<Command>,

    #[knus(child)]
    pub double_click: Option<Command>,

    #[knus(child)]
    pub mouse_right: Option<Command>,

    #[knus(child)]
    pub mouse_middle: Option<Command>,

    #[knus(child)]
    pub scroll_up: Option<Command>,

    #[knus(child)]
    pub scroll_down: Option<Command>,

    #[knus(child)]
    pub scroll_right: Option<Command>,

    #[knus(child)]
    pub scroll_left: Option<Command>,
}

impl MouseBinds {
    fn hydrate(self) -> HydratedMouseBinds {
        fn process_command(cmd: Option<Command>) -> Option<Message> {
            let Some(cmd) = cmd else {
                return None;
            };
            if cmd.args.is_empty() {
                None
            } else if let Some(sh) = cmd.sh
                && sh
            {
                Some(Message::Command(CommandSpec {
                    command: String::from("sh"),
                    args: Some(vec![String::from("-c"), cmd.args[0].clone()]),
                }))
            } else {
                Some(Message::Command(CommandSpec {
                    command: cmd.args[0].clone(),
                    args: cmd.args.get(1..).map(<[String]>::to_vec),
                }))
            }
        }

        HydratedMouseBinds {
            mouse_left: process_command(self.mouse_left),
            double_click: process_command(self.double_click),
            mouse_right: process_command(self.mouse_right),
            mouse_middle: process_command(self.mouse_middle),
            scroll: if self.scroll_up.is_some()
                || self.scroll_down.is_some()
                || self.scroll_left.is_some()
                || self.scroll_right.is_some()
            {
                Some(Scroll {
                    up: process_command(self.scroll_up),
                    down: process_command(self.scroll_down),
                    right: process_command(self.scroll_right),
                    left: process_command(self.scroll_left),
                })
            } else {
                None
            },
        }
    }
}

pub struct HydratedMouseBinds {
    pub mouse_left: Option<Message>,
    pub double_click: Option<Message>,
    pub mouse_right: Option<Message>,
    pub mouse_middle: Option<Message>,
    pub scroll: Option<Scroll>,
}

pub struct Scroll {
    pub up: Option<Message>,
    pub down: Option<Message>,
    pub right: Option<Message>,
    pub left: Option<Message>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MediaControl {
    Play,
    Pause,
    PlayPause,
    Stop,
    Next,
    Previous,
    Seek(i64),
    Volume(f64),
    SetVolume(f64),
}

impl<S> knus::Decode<S> for MediaControl
where
    S: knus::traits::ErrorSpan,
{
    fn decode_node(
        node: &knus::ast::SpannedNode<S>,
        ctx: &mut knus::decode::Context<S>,
    ) -> Result<Self, DecodeError<S>> {
        if let Some(type_name) = &node.type_name {
            ctx.emit_error(DecodeError::unexpected(
                type_name,
                "type name",
                "no type name expected for this node",
            ));
        }

        for name in node.properties.keys() {
            ctx.emit_error(DecodeError::unexpected(
                name,
                "property",
                format!("unexpected property `{0}`", name.escape_default()),
            ));
        }

        if let Some(children) = &node.children {
            for child in children.iter() {
                ctx.emit_error(DecodeError::unexpected(
                    child,
                    "node",
                    format!(
                        "unexpected node `{0}`",
                        child.node_name.escape_default(),
                    ),
                ));
            }
        }

        let mut iter_args = node.arguments.iter();
        let Some(first_arg) = iter_args.next() else {
            return Err(DecodeError::missing(
                node,
                "expected additional argument",
            ));
        };
        match &*first_arg.literal {
            Literal::String(arg) => match arg.as_ref() {
                "play" => Ok(MediaControl::Play),
                "pause" => Ok(MediaControl::Pause),
                "play-pause" => Ok(MediaControl::PlayPause),
                "stop" => Ok(MediaControl::Stop),
                "next" => Ok(MediaControl::Next),
                "previous" => Ok(MediaControl::Previous),
                "seek" => {
                    let Some(second_arg) = iter_args.next() else {
                        return Err(DecodeError::missing(
                            node,
                            "seek requires additional argument",
                        ));
                    };

                    match &*second_arg.literal {
                        Literal::Int(seek_amount) => {
                            match i64::try_from(seek_amount) {
                                Ok(seek_amount) => Ok(MediaControl::Seek(
                                    // convert from microseconds to millseconds
                                    seek_amount * 1000,
                                )),
                                Err(e) => Err(DecodeError::conversion(
                                    &second_arg.literal,
                                    format!("{e}"),
                                )),
                            }
                        }
                        _other => Err(DecodeError::scalar_kind(
                            Kind::Int,
                            &second_arg.literal,
                        )),
                    }
                }
                "volume" => {
                    let Some(second_arg) = iter_args.next() else {
                        return Err(DecodeError::missing(
                            node,
                            "volume requires additional argument",
                        ));
                    };

                    match &*second_arg.literal {
                        Literal::Decimal(volume_amount) => {
                            match f64::try_from(volume_amount) {
                                Ok(amount) => Ok(MediaControl::Volume(amount)),

                                Err(e) => Err(DecodeError::conversion(
                                    &second_arg.literal,
                                    format!("{e}"),
                                )),
                            }
                        }

                        _other => Err(DecodeError::scalar_kind(
                            Kind::Decimal,
                            &second_arg.literal,
                        )),
                    }
                }
                "set-volume" => {
                    let Some(second_arg) = iter_args.next() else {
                        return Err(DecodeError::missing(
                            node,
                            "set-volume requires additional argument",
                        ));
                    };

                    match &*second_arg.literal {
                        Literal::Decimal(volume_amount) => {
                            match f64::try_from(volume_amount) {
                                Ok(amount) => {
                                    Ok(MediaControl::SetVolume(amount))
                                }

                                Err(e) => Err(DecodeError::conversion(
                                    &second_arg.literal,
                                    format!("{e}"),
                                )),
                            }
                        }

                        _other => Err(DecodeError::scalar_kind(
                            Kind::Decimal,
                            &second_arg.literal,
                        )),
                    }
                }
                _other => Err(DecodeError::conversion(
                    &node.node_name,
                    "expected `play`, `pause`, `play-pause`, `stop`, `next`, `previous`, `seek`, `volume`, or `set-volume`",
                )),
            },
            _other => {
                Err(DecodeError::scalar_kind(Kind::String, &first_arg.literal))
            }
        }
    }
}

#[derive(knus::Decode, Debug, Clone, Default)]
pub struct MouseBindsForMpris {
    #[knus(child)]
    pub mouse_left: Option<MediaControl>,

    #[knus(child)]
    pub double_click: Option<MediaControl>,

    #[knus(child)]
    pub mouse_right: Option<MediaControl>,

    #[knus(child)]
    pub mouse_middle: Option<MediaControl>,

    #[knus(child)]
    pub scroll_up: Option<MediaControl>,

    #[knus(child)]
    pub scroll_down: Option<MediaControl>,

    #[knus(child)]
    pub scroll_right: Option<MediaControl>,

    #[knus(child)]
    pub scroll_left: Option<MediaControl>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Command {
    pub sh: Option<bool>,

    pub args: Vec<String>,
}

#[derive(knus::Decode, Debug, Clone, Default)]
pub struct ContainerStyle {
    #[knus(child, unwrap(argument))]
    pub text_color: Option<ConfigColor>,
    #[knus(child, unwrap(argument))]
    pub background: Option<ConfigColor>,
    #[knus(child)]
    pub border: Option<ConfigBorder>,
    #[knus(child, unwrap(argument))]
    pub padding: Option<f32>,
}

impl ContainerStyle {
    fn hydrate(self, color_vars: &ColorVars) -> HydratedContainerStyle {
        HydratedContainerStyle {
            style: container::Style {
                text_color: self.text_color.map(|c| c.resolve(color_vars)),
                background: {
                    if let Some(bg_color) =
                        self.background.map(|c| c.resolve(color_vars))
                    {
                        Some(Background::Color(bg_color))
                    } else {
                        None
                    }
                },
                border: self
                    .border
                    .map(|b| to_iced_border(b, color_vars))
                    .unwrap_or(iced::Border::default()),
                ..Default::default()
            },
            padding: self.padding,
        }
    }
}

#[derive(Default, Clone)]
pub struct HydratedContainerStyle {
    pub style: container::Style,
    pub padding: Option<f32>,
}

impl Deref for HydratedContainerStyle {
    type Target = container::Style;

    fn deref(&self) -> &Self::Target {
        &self.style
    }
}

impl DerefMut for HydratedContainerStyle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.style
    }
}

#[derive(knus::Decode, Debug, Clone, Default)]
pub struct ConfigBorder {
    #[knus(child, unwrap(argument))]
    pub color: Option<ConfigColor>,
    #[knus(child, unwrap(argument), default)]
    pub width: Option<f32>,
    #[knus(child)]
    pub radius: Option<ConfigRadius>,
}

fn to_iced_border(
    border: ConfigBorder,
    color_vars: &ColorVars,
) -> iced::Border {
    let default_border = iced::Border::default();
    iced::Border {
        color: border
            .color
            .unwrap_or(default_border.color.into())
            .resolve(&color_vars),
        width: border.width.unwrap_or(default_border.width),
        radius: match border.radius {
            Some(ConfigRadius::All(r)) => iced::border::radius(r),
            Some(ConfigRadius::PerCorner(PerCorner {
                top_left,
                top_right,
                bottom_left,
                bottom_right,
            })) => iced::border::Radius {
                top_left,
                top_right,
                bottom_right,
                bottom_left,
            },
            None => default_border.radius,
        },
    }
}

#[derive(Debug, Clone)]
pub enum ConfigRadius {
    All(f32),
    PerCorner(PerCorner),
}

impl From<ConfigRadius> for iced::border::Radius {
    fn from(radius: ConfigRadius) -> Self {
        match radius {
            ConfigRadius::All(r) => iced::border::radius(r),
            ConfigRadius::PerCorner(corners) => iced::border::Radius {
                top_left: corners.top_left,
                top_right: corners.top_right,
                bottom_left: corners.bottom_left,
                bottom_right: corners.bottom_right,
            },
        }
    }
}

#[derive(knus::Decode, Debug, Clone, Default)]
pub struct PerCorner {
    #[knus(child, unwrap(argument))]
    top_left: f32,
    #[knus(child, unwrap(argument))]
    top_right: f32,
    #[knus(child, unwrap(argument))]
    bottom_left: f32,
    #[knus(child, unwrap(argument))]
    bottom_right: f32,
}

impl<S> knus::Decode<S> for ConfigRadius
where
    S: knus::traits::ErrorSpan,
{
    fn decode_node(
        node: &knus::ast::SpannedNode<S>,
        ctx: &mut knus::decode::Context<S>,
    ) -> Result<Self, DecodeError<S>> {
        if let Some(type_name) = &node.type_name {
            ctx.emit_error(DecodeError::unexpected(
                type_name,
                "type name",
                "no type name expected for this node",
            ));
        }

        node.properties.iter().for_each(|property| {
            ctx.emit_error(DecodeError::unexpected(
                property.0,
                "property",
                "no propertes expected for this node",
            ));
        });

        let mut iter_args = node.arguments.iter();
        let len = iter_args.len();
        if len == 0 {
            if node.children.iter().len() == 0 {
                Err(DecodeError::missing(
                    node,
                    "additional argument `radius` is required",
                ))
            } else {
                let per_corner = PerCorner::decode_node(node, ctx)?;
                Ok(Self::PerCorner(per_corner))
            }
        } else if len == 1 {
            node.children.iter().for_each(|child| {
                ctx.emit_error(DecodeError::unexpected(
                    child,
                    "node",
                    "no children expected when radius is specified as an argument",
                ));
            });
            let radius = iter_args.next().unwrap();
            Ok(Self::All(f32::decode(radius, ctx)?))
        } else {
            iter_args.for_each(|arg| {
                ctx.emit_error(DecodeError::unexpected(
                    &arg.literal,
                    "argument",
                    "expected 1 or 0 arguments",
                ));
            });

            let per_corner = PerCorner::decode_node(node, ctx)?;
            Ok(Self::PerCorner(per_corner))
        }
    }
}

impl<S> knus::Decode<S> for Command
where
    S: knus::traits::ErrorSpan,
{
    fn decode_node(
        node: &knus::ast::SpannedNode<S>,
        ctx: &mut knus::decode::Context<S>,
    ) -> Result<Self, DecodeError<S>> {
        if let Some(type_name) = &node.type_name {
            ctx.emit_error(DecodeError::unexpected(
                type_name,
                "type name",
                "no type name expected for this node",
            ));
        }

        let mut sh = None;
        for (name, val) in &node.properties {
            match &***name {
                "sh" => {
                    sh = knus::traits::DecodeScalar::decode(val, ctx)?;
                }
                name_str => {
                    return Err(DecodeError::unexpected(
                        name,
                        "property",
                        format!(
                            "unexpected property `{0}`",
                            name_str.escape_default(),
                        ),
                    ));
                }
            }
        }

        let mut iter_args = node.arguments.iter();
        if iter_args.len() > 1
            && let Some(sh) = sh
            && sh
        {
            return Err(DecodeError::unexpected(
                &iter_args.nth(1).unwrap().literal,
                "argument",
                "when sh=true, only 1 argument is allowed",
            ));
        }
        let args = iter_args
            .map(|val| knus::traits::DecodeScalar::decode(val, ctx))
            .collect::<Result<_, _>>()?;
        let children = node.children.as_ref().map_or(&[][..], |lst| &lst[..]);
        for child in children {
            let name_str = &**child.node_name;
            ctx.emit_error(DecodeError::unexpected(
                child,
                "node",
                format!("unexpected node `{0}`", name_str.escape_default(),),
            ));
        }
        Ok(Command { sh, args })
    }
}

impl ColorVars {
    pub fn load(path: &Path) -> miette::Result<Self> {
        let contents = fs::read_to_string(path)
            .into_diagnostic()
            .with_context(|| format!("error reading {}", path.display()))?;

        let colors = Self::parse(
            path.file_name()
                .and_then(OsStr::to_str)
                .unwrap_or("config.kdl"),
            &contents,
        )?;

        Ok(colors)
    }

    pub fn get(&self, name: &str) -> Option<Color> {
        let name = name.strip_prefix('$').unwrap_or(name);
        self.vars.get(name).cloned()
    }

    pub fn parse(filename: &str, text: &str) -> miette::Result<Self> {
        match knus::parse::<ColorVars>(filename, text) {
            Ok(colors) => {
                debug!("Successfully parsed colors");
                Ok(colors)
            }
            Err(e) => Err(miette::Report::new(e)),
        }
    }
}

#[profiling::all_functions]
impl Config {
    pub fn load(path: &Path) -> miette::Result<Self> {
        let contents = fs::read_to_string(path)
            .into_diagnostic()
            .with_context(|| format!("error reading {}", path.display()))?;

        let config = Self::parse(
            path.file_name()
                .and_then(OsStr::to_str)
                .unwrap_or("config.kdl"),
            &contents,
        )?;

        Ok(config)
    }

    pub fn parse(filename: &str, text: &str) -> miette::Result<Self> {
        match knus::parse::<Config>(filename, text) {
            Ok(config) => {
                info!("Successfully parsed config");
                Ok(config)
            }
            Err(e) => Err(miette::Report::new(e)),
        }
    }

    pub fn create(path: &Path) -> miette::Result<()> {
        if let Some(default_parent) = path.parent() {
            fs::create_dir_all(default_parent)
                .into_diagnostic()
                .with_context(|| {
                    format!(
                        "error creating config directory {}",
                        default_parent.display()
                    )
                })?;
        }

        let mut new_file = match File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)
        {
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                return Ok(());
            }
            res => res,
        }
        .into_diagnostic()
        .with_context(|| {
            format!("error opening config file at {}", path.display())
        })?;

        let default_config = include_bytes!("../assets/default-config.kdl");

        new_file
            .write_all(default_config)
            .into_diagnostic()
            .with_context(|| {
                format!("error writing default config to {}", path.display())
            })?;

        Ok(())
    }

    pub fn load_or_create(path: &Path) -> miette::Result<Self> {
        Config::create(path)?;
        Config::load(path)
    }

    pub fn init() -> (HydratedConfig, ColorVars, ConfigPath, PathBuf) {
        let Some(project_dir) = ProjectDirs::from("", "", BAR_NAMESPACE) else {
            std::process::exit(1);
        };

        let config_dir = project_dir.config_dir().to_path_buf();

        let colors_path = config_dir.join("colors.kdl");
        let colors = {
            match ColorVars::load(&colors_path) {
                Err(e) => {
                    if let Err(e) = Notification::new()
                        .summary(BAR_NAMESPACE)
                        .body("Failed to parse colors file")
                        .show()
                    {
                        error!("{e}");
                    }
                    error!("Failed to parse colors file ",);
                    error!("{e:?}");
                    ColorVars::default()
                }
                Ok(colors) => colors,
            }
        };

        let config_path = config_dir.join("config.kdl");
        let config = {
            match Config::load_or_create(&config_path) {
                Err(e) => {
                    if let Err(e) = Notification::new()
                        .summary(BAR_NAMESPACE)
                        .body(
                            "Failed to parse config file, using default config",
                        )
                        .show()
                    {
                        error!("{e}");
                    }
                    error!("Failed to parse config file, using default config");
                    error!("{e:?}");
                    Config::default()
                }
                Ok(config) => config,
            }
        };

        let hydrated_config = config.hydrate(&colors);

        let path = ConfigPath {
            config: config_path,
            colors: colors_path,
        };

        (hydrated_config, colors, path, config_dir)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfigColor {
    Literal(Color),
    Variable(String),
}

impl ConfigColor {
    pub fn resolve(&self, colors: &ColorVars) -> Color {
        match self {
            ConfigColor::Literal(c) => *c,
            ConfigColor::Variable(name) => {
                colors.get(name).unwrap_or_else(|| {
                    error!(
                        "Color variable '{}' not found, using red as default",
                        name
                    );
                    Color::from_rgb(1.0, 0.0, 0.0)
                })
            }
        }
    }

    pub fn parse(&self) -> Color {
        match self {
            ConfigColor::Literal(c) => *c,
            ConfigColor::Variable(name) => {
                error!(
                    "Color variable '{}' not found, using red as default",
                    name
                );
                Color::from_rgb(1.0, 0.0, 0.0)
            }
        }
    }
}

impl Default for ConfigColor {
    fn default() -> Self {
        Self::Literal(Color::from_rgb(1.0, 0.0, 0.0))
    }
}

impl From<Color> for ConfigColor {
    fn from(color: Color) -> Self {
        Self::Literal(color)
    }
}

impl<S> knus::DecodeScalar<S> for ConfigColor
where
    S: knus::traits::ErrorSpan,
{
    fn type_check(
        type_name: &Option<knus::span::Spanned<knus::ast::TypeName, S>>,
        ctx: &mut knus::decode::Context<S>,
    ) {
        if let Some(type_name) = &type_name {
            ctx.emit_error(DecodeError::unexpected(
                type_name,
                "type name",
                "no type name expected for this node",
            ));
        }
    }

    fn raw_decode(
        value: &knus::span::Spanned<Literal, S>,
        _ctx: &mut knus::decode::Context<S>,
    ) -> Result<Self, DecodeError<S>> {
        match **value {
            knus::ast::Literal::String(ref s) => {
                if s.starts_with('$') {
                    Ok(ConfigColor::Variable(s.to_string()))
                } else {
                    let color = Color::parse(s).ok_or_else(|| {
                        DecodeError::conversion(value, "invalid hex literal")
                    })?;
                    Ok(ConfigColor::Literal(color))
                }
            }
            _ => Err(DecodeError::conversion(value, "invalid hex literal")),
        }
    }
}
