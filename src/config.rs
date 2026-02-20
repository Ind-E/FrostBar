use std::{
    ffi::OsStr,
    fs::{self, File},
    io::Write,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    str::FromStr,
};

use iced::{Background, Color, border, color, widget::container};
use knuffel::{
    Decode, DecodeScalar, ast::Literal, decode::Kind, errors::DecodeError,
};
use miette::{Context, IntoDiagnostic};
use owo_colors::OwoColorize;
use rustc_hash::FxHashMap;
use tracing::{debug, error, info};

use crate::{
    CommandSpec, Message,
    file_watcher::ConfigPath,
    modules::{BarAlignment, BarPosition, ModuleMsg, niri::service::NiriEvent},
    utils::{log::notification, niri::config_to_ipc_action},
};

const DEFAULT_CONFIG: &[u8] = include_bytes!("../assets/default-config.kdl");

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FloatOrPercent {
    Percent(f32),
    Float(f32),
}

impl Default for FloatOrPercent {
    fn default() -> Self {
        FloatOrPercent::Float(0.0)
    }
}

impl<S: knuffel::traits::ErrorSpan> knuffel::DecodeScalar<S>
    for FloatOrPercent
{
    fn type_check(
        type_name: &Option<knuffel::span::Spanned<knuffel::ast::TypeName, S>>,
        ctx: &mut knuffel::decode::Context<S>,
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
        val: &knuffel::span::Spanned<knuffel::ast::Literal, S>,
        ctx: &mut knuffel::decode::Context<S>,
    ) -> Result<Self, DecodeError<S>> {
        match &**val {
            knuffel::ast::Literal::Int(value) => {
                let v = value.try_into().unwrap_or_else(|e| {
                    ctx.emit_error(DecodeError::unsupported(
                        val,
                        format!("{e}"),
                    ));
                    i32::default()
                });
                Ok(FloatOrPercent::Float(v as f32))
            }
            knuffel::ast::Literal::Decimal(value) => {
                let v = value.try_into().unwrap_or_else(|e| {
                    ctx.emit_error(DecodeError::unsupported(
                        val,
                        format!("{e}"),
                    ));
                    f32::default()
                });
                Ok(FloatOrPercent::Float(v))
            }
            knuffel::ast::Literal::String(value) => {
                if value.ends_with('%') {
                    match value.trim_end_matches('%').parse::<f32>() {
                        Ok(v) => {
                            if (0.0..=100.0).contains(&v) {
                                Ok(FloatOrPercent::Percent(v / 100.0))
                            } else {
                                ctx.emit_error(DecodeError::unsupported(
                                    val,
                                    "percent must be between 0 and 100",
                                ));
                                Ok(FloatOrPercent::default())
                            }
                        }
                        Err(e) => {
                            ctx.emit_error(DecodeError::unsupported(
                                val,
                                format!("{e}"),
                            ));
                            Ok(FloatOrPercent::default())
                        }
                    }
                } else {
                    ctx.emit_error(DecodeError::unsupported(
                        val,
                        "expected string to end with `%`",
                    ));
                    Ok(FloatOrPercent::default())
                }
            }
            _ => {
                ctx.emit_error(DecodeError::unsupported(
                    val,
                    "Unsupported value, only numbers are recognized",
                ));
                Ok(FloatOrPercent::default())
            }
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct FloatOrInt<const MIN: i32, const MAX: i32>(pub f32);

impl<const MIN: i32, const MAX: i32> FloatOrInt<MIN, MAX> {
    fn into_f32(self) -> f32 {
        self.0
    }
}

impl<const MIN: i32, const MAX: i32> From<f32> for FloatOrInt<MIN, MAX> {
    fn from(value: f32) -> Self {
        assert!(value >= MIN as f32 && value <= MAX as f32, "out of range");
        FloatOrInt(value)
    }
}

impl<const MIN: i32, const MAX: i32> From<FloatOrInt<MIN, MAX>> for f32 {
    fn from(value: FloatOrInt<MIN, MAX>) -> Self {
        value.0
    }
}

impl<S: knuffel::traits::ErrorSpan, const MIN: i32, const MAX: i32>
    knuffel::DecodeScalar<S> for FloatOrInt<MIN, MAX>
{
    fn type_check(
        type_name: &Option<knuffel::span::Spanned<knuffel::ast::TypeName, S>>,
        ctx: &mut knuffel::decode::Context<S>,
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
        val: &knuffel::span::Spanned<knuffel::ast::Literal, S>,
        ctx: &mut knuffel::decode::Context<S>,
    ) -> Result<Self, DecodeError<S>> {
        match &**val {
            knuffel::ast::Literal::Int(value) => match value.try_into() {
                Ok(v) => {
                    if (MIN..=MAX).contains(&v) {
                        Ok(FloatOrInt(v as f32))
                    } else {
                        ctx.emit_error(DecodeError::unsupported(
                            val,
                            format!("value must be between {MIN} and {MAX}"),
                        ));
                        Ok(FloatOrInt::default())
                    }
                }
                Err(e) => {
                    ctx.emit_error(DecodeError::unsupported(
                        val,
                        format!("{e}"),
                    ));
                    Ok(FloatOrInt::default())
                }
            },
            knuffel::ast::Literal::Decimal(value) => match value.try_into() {
                Ok(v) => {
                    if ((MIN as f32)..=(MAX as f32)).contains(&v) {
                        Ok(FloatOrInt(v))
                    } else {
                        ctx.emit_error(DecodeError::unsupported(
                            val,
                            format!("value must be between {MIN} and {MAX}"),
                        ));
                        Ok(FloatOrInt::default())
                    }
                }
                Err(e) => {
                    ctx.emit_error(DecodeError::unsupported(
                        val,
                        format!("{e}"),
                    ));
                    Ok(FloatOrInt::default())
                }
            },
            _ => {
                ctx.emit_error(DecodeError::unsupported(
                    val,
                    "Unsupported value, only numbers are recognized",
                ));
                Ok(FloatOrInt::default())
            }
        }
    }
}

#[derive(knuffel::Decode, Debug)]
pub struct RawConfig {
    #[knuffel(child, default)]
    layout: Layout,
    #[knuffel(child, default)]
    pub style: RawTopLevelStyle,
    #[knuffel(child, default)]
    pub start: Start,
    #[knuffel(child, default)]
    pub middle: Middle,
    #[knuffel(child, default)]
    pub end: End,
}

impl Default for RawConfig {
    fn default() -> Self {
        RawConfig::parse("", str::from_utf8(DEFAULT_CONFIG).unwrap()).unwrap()
    }
}

pub struct Config {
    pub layout: Layout,
    pub style: TopLevelStyle,
    pub modules: ConfigModules,
}

impl RawConfig {
    pub fn hydrate(self, colors: &ColorVars) -> Config {
        Config {
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
) -> ConfigModules {
    let mut modules = Vec::new();

    let mut process_section =
        |mut module_configs: Vec<RawConfigModule>, align: BarAlignment| {
            for (idx, module_config) in module_configs.drain(..).enumerate() {
                let position = BarPosition { idx, align };
                match module_config {
                    RawConfigModule::Battery(c) => {
                        modules.push((c.hydrate(colors), position));
                    }
                    RawConfigModule::AudioVisualizer(c) => {
                        modules.push((c.hydrate(colors), position));
                    }
                    RawConfigModule::Mpris(c) => {
                        modules.push((c.hydrate(colors), position));
                    }
                    RawConfigModule::Time(c) => {
                        modules.push((c.hydrate(colors), position));
                    }
                    RawConfigModule::Label(c) => {
                        modules.push((c.hydrate(colors), position));
                    }
                    RawConfigModule::Niri(c) => {
                        modules.push((c.hydrate(colors), position));
                    }
                    RawConfigModule::SystemTray(c) => {
                        modules.push((c.hydrate(colors), position));
                    }
                }
            }
        };

    process_section(value.0.modules, BarAlignment::Start);
    process_section(value.1.modules, BarAlignment::Middle);
    process_section(value.2.modules, BarAlignment::End);

    ConfigModules { inner: modules }
}

#[derive(knuffel::Decode, Debug, Default)]
pub struct Start {
    #[knuffel(children, default)]
    pub modules: Vec<RawConfigModule>,
}

#[derive(knuffel::Decode, Debug, Default)]
pub struct Middle {
    #[knuffel(children, default)]
    pub modules: Vec<RawConfigModule>,
}

#[derive(knuffel::Decode, Debug, Default)]
pub struct End {
    #[knuffel(children, default)]
    pub modules: Vec<RawConfigModule>,
}

#[derive(knuffel::Decode, Debug, Clone, PartialEq)]
pub struct Layout {
    #[knuffel(child, unwrap(argument), default = 42)]
    pub width: u32,
    #[knuffel(child, unwrap(argument), default = 0)]
    pub gaps: i32,
    #[knuffel(child, unwrap(argument), default = Self::default().anchor)]
    pub anchor: Anchor,
    #[knuffel(child, unwrap(argument), default = Self::default().layer)]
    pub layer: Layer,
}

#[derive(knuffel::DecodeScalar, Debug, Clone, Copy, PartialEq)]
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

#[derive(knuffel::DecodeScalar, Debug, Clone, Copy, PartialEq)]
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

#[derive(knuffel::Decode, Debug, Clone)]
pub struct RawTopLevelStyle {
    #[knuffel(child, default = Self::default().border_radius)]
    pub border_radius: RawConfigRadius,
    #[knuffel(child, unwrap(argument), default = Self::default().background)]
    pub background: ConfigColor,
}

impl Default for RawTopLevelStyle {
    fn default() -> Self {
        Self {
            border_radius: RawConfigRadius::All(0.0.into()),
            background: Color::from_rgb(0.0, 0.0, 0.0).into(),
        }
    }
}

pub struct TopLevelStyle {
    pub border_radius: border::Radius,
    pub background: Color,
}

impl RawTopLevelStyle {
    fn hydrate(self, colors: &ColorVars) -> TopLevelStyle {
        TopLevelStyle {
            border_radius: self.border_radius.into(),
            background: self.background.resolve(colors),
        }
    }
}

#[derive(knuffel::Decode, Debug)]
struct ColorVariable {
    #[knuffel(node_name)]
    pub name: String,
    #[knuffel(argument)]
    pub color: ConfigColor,
}

#[derive(Debug, Default)]
pub struct ColorVars {
    vars: FxHashMap<String, Color>,
}

impl<S> knuffel::DecodeChildren<S> for ColorVars
where
    S: knuffel::traits::ErrorSpan,
{
    fn decode_children(
        nodes: &[knuffel::ast::SpannedNode<S>],
        ctx: &mut knuffel::decode::Context<S>,
    ) -> Result<Self, DecodeError<S>> {
        let mut vars = FxHashMap::default();
        for node in nodes {
            let var = ColorVariable::decode_node(node, ctx)?;
            vars.insert(var.name, var.color.parse());
        }

        Ok(Self { vars })
    }
}

#[derive(knuffel::Decode, Debug)]
pub enum RawConfigModule {
    AudioVisualizer(RawAudioVisualizer),
    Battery(RawBattery),
    Time(RawTime),
    Mpris(RawMpris),
    Niri(Box<RawNiri>),
    Label(RawLabel),
    SystemTray(RawSystemTray),
}

pub enum ConfigModule {
    AudioVisualizer(AudioVisualizer),
    Battery(Battery),
    Time(Time),
    Mpris(Mpris),
    Niri(Box<Niri>),
    Label(Label),
    SystemTray(SystemTray),
}

pub struct ConfigModules {
    inner: Vec<(ConfigModule, BarPosition)>,
}

impl std::ops::Deref for ConfigModules {
    type Target = Vec<(ConfigModule, BarPosition)>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::ops::DerefMut for ConfigModules {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[derive(knuffel::Decode, Debug)]
pub struct RawAudioVisualizer {
    #[knuffel(child, unwrap(argument), default = Self::default().length)]
    pub length: u32,

    #[knuffel(child, unwrap(argument), default = Self::default().spacing)]
    pub spacing: FloatOrPercent,

    #[knuffel(child, unwrap(argument), default = Self::default().color)]
    pub color: ConfigColor,

    #[knuffel(child, unwrap(argument), default = Self::default().dynamic_color)]
    pub dynamic_color: bool,

    #[knuffel(flatten(child), default)]
    pub binds: RawMouseBinds,

    #[knuffel(child, default)]
    pub style: RawContainerStyle,
}

impl Default for RawAudioVisualizer {
    fn default() -> Self {
        Self {
            length: 130,
            spacing: FloatOrPercent::Percent(0.1),
            dynamic_color: true,
            color: Color::WHITE.into(),
            binds: RawMouseBinds::default(),
            style: RawContainerStyle::default(),
        }
    }
}

impl RawAudioVisualizer {
    fn hydrate(self, colors: &ColorVars) -> ConfigModule {
        let audio_visualizer = AudioVisualizer {
            length: self.length,
            spacing: self.spacing,
            color: self.color.resolve(colors),
            dynamic_color: self.dynamic_color,
            binds: self.binds.hydrate(),
            style: self.style.hydrate(colors),
        };
        ConfigModule::AudioVisualizer(audio_visualizer)
    }
}

#[derive(Debug, Clone)]
pub struct AudioVisualizer {
    pub length: u32,
    pub spacing: FloatOrPercent,
    pub color: Color,
    pub dynamic_color: bool,
    pub binds: MouseBinds,
    pub style: ContainerStyle,
}

#[derive(knuffel::Decode, Debug)]
pub struct RawBattery {
    #[knuffel(child, unwrap(argument), default = Self::default().icon_size)]
    pub icon_size: u32,

    #[knuffel(child, unwrap(argument), default = Self::default().charging_color)]
    pub charging_color: ConfigColor,

    #[knuffel(child, default)]
    pub style: RawContainerStyle,

    #[knuffel(flatten(child), default)]
    pub binds: RawMouseBinds,
}

impl Default for RawBattery {
    fn default() -> Self {
        Self {
            icon_size: 22,
            charging_color: color!(0x73F5AB).into(),
            style: RawContainerStyle::default(),
            binds: RawMouseBinds::default(),
        }
    }
}

impl RawBattery {
    fn hydrate(self, colors: &ColorVars) -> ConfigModule {
        let battery = Battery {
            icon_size: self.icon_size,
            charging_color: self.charging_color.resolve(colors),
            style: self.style.hydrate(colors),
            binds: self.binds.hydrate(),
        };

        ConfigModule::Battery(battery)
    }
}

pub struct Battery {
    pub icon_size: u32,
    pub charging_color: Color,
    pub style: ContainerStyle,
    pub binds: MouseBinds,
}

#[derive(knuffel::Decode, Debug)]
pub struct RawTime {
    #[knuffel(child, unwrap(argument), default = "%I\n%M".to_string())]
    pub format: String,

    #[knuffel(child, unwrap(argument), default = "%a %b %-d\n%-m/%-d/%y".to_string())]
    pub tooltip_format: String,

    #[knuffel(flatten(child), default)]
    pub binds: RawMouseBinds,

    #[knuffel(child, default)]
    pub style: RawContainerStyle,
}

impl RawTime {
    fn hydrate(self, colors: &ColorVars) -> ConfigModule {
        let time = Time {
            format: self.format,
            tooltip_format: self.tooltip_format,
            binds: self.binds.hydrate(),
            style: self.style.hydrate(colors),
        };

        ConfigModule::Time(time)
    }
}

pub struct Time {
    pub format: String,
    pub tooltip_format: String,
    pub binds: MouseBinds,
    pub style: ContainerStyle,
}

#[derive(knuffel::Decode, Debug, Clone)]
pub struct RawMpris {
    #[knuffel(child, unwrap(argument), default = "󰝚".to_string())]
    pub placeholder: String,

    #[knuffel(flatten(child), default)]
    pub binds: RawMouseBindsForMpris,

    #[knuffel(child, default)]
    pub placeholder_style: RawContainerStyle,
}

impl RawMpris {
    fn hydrate(self, colors: &ColorVars) -> ConfigModule {
        let mpris = Mpris {
            placeholder: self.placeholder,
            binds: self.binds.into(),
            placeholder_style: self.placeholder_style.hydrate(colors),
        };

        ConfigModule::Mpris(mpris)
    }
}

pub struct Mpris {
    pub placeholder: String,
    pub binds: MouseBindsForMpris,
    pub placeholder_style: ContainerStyle,
}

#[derive(knuffel::Decode, Debug, Clone)]
pub struct RawNiri {
    #[knuffel(child, unwrap(argument), default = 10)]
    spacing: u32,

    #[knuffel(child, unwrap(argument), default = 0)]
    workspace_offset: i8,

    #[knuffel(child, default)]
    style: RawContainerStyle,

    #[knuffel(child, default)]
    window_focused_style: RawContainerStyle,

    #[knuffel(child, default)]
    window_style: RawContainerStyle,

    #[knuffel(child, default)]
    workspace_active_style: RawContainerStyle,

    #[knuffel(child, default)]
    workspace_hovered_style: RawContainerStyle,

    #[knuffel(child, default)]
    workspace_style: RawContainerStyle,

    #[knuffel(flatten(child), default)]
    pub binds: RawMouseBinds,
}

impl RawNiri {
    fn hydrate(self, colors: &ColorVars) -> ConfigModule {
        let workspace_base_style = self.workspace_style.hydrate(colors);

        let mut workspace_hovered_style = workspace_base_style.clone();
        if let Some(text_color) = &self
            .workspace_hovered_style
            .text_color
            .map(|c| c.resolve(colors))
        {
            workspace_hovered_style.text_color = Some(*text_color);
        }
        if let Some(background) = &self
            .workspace_hovered_style
            .background
            .map(|c| c.resolve(colors))
        {
            workspace_hovered_style.background =
                Some(Background::Color(*background));
        }
        if let Some(border) = &self.workspace_hovered_style.border {
            let mut hovered_border = iced::Border::default();
            if let Some(color) =
                border.color.as_ref().map(|c| c.resolve(colors))
            {
                hovered_border.color = color;
            }
            if let Some(width) = border.width {
                hovered_border.width = width.into();
            }
            if let Some(radius) = &border.radius {
                hovered_border.radius = radius.clone().into();
            }
            workspace_hovered_style.border = hovered_border;
        }

        let mut workspace_active_style = workspace_base_style.clone();
        let mut workspace_active_hovered_style =
            workspace_hovered_style.clone();
        if let Some(text_color) = &self
            .workspace_active_style
            .text_color
            .map(|c| c.resolve(colors))
        {
            workspace_active_style.text_color = Some(*text_color);
            workspace_active_hovered_style.text_color = Some(*text_color);
        }
        if let Some(background) = &self
            .workspace_active_style
            .background
            .map(|c| c.resolve(colors))
        {
            workspace_active_style.background =
                Some(Background::Color(*background));
            workspace_active_hovered_style.background =
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
                active_border.width = width.into();
                active_hovered_border.width = width.into();
            }
            if let Some(radius) = &border.radius {
                active_border.radius = radius.clone().into();
                active_hovered_border.radius = radius.clone().into();
            }
            workspace_active_style.border = active_border;
            workspace_active_hovered_style.border = active_hovered_border;
        }

        let workspace_active_hovered_style_merged =
            workspace_active_hovered_style;
        let workspace_active_style_merged = workspace_active_style;
        let workspace_hovered_style_merged = workspace_hovered_style;

        let window_base_style = self.window_style.hydrate(colors);
        let mut window_focused_style = window_base_style.clone();

        if let Some(text_color) = &self
            .window_focused_style
            .text_color
            .map(|c| c.resolve(colors))
        {
            window_focused_style.text_color = Some(*text_color);
            window_focused_style.text_color = Some(*text_color);
        }
        if let Some(background) = &self
            .window_focused_style
            .background
            .map(|c| c.resolve(colors))
        {
            window_focused_style.background =
                Some(Background::Color(*background));
            window_focused_style.background =
                Some(Background::Color(*background));
        }

        if let Some(border) = &self.window_focused_style.border {
            let mut focused_border = iced::Border::default();
            if let Some(color) =
                border.color.as_ref().map(|c| c.resolve(colors))
            {
                focused_border.color = color;
            }
            if let Some(width) = border.width {
                focused_border.width = width.into();
            }
            if let Some(radius) = &border.radius {
                focused_border.radius = radius.clone().into();
            }
            window_focused_style.border = focused_border;
        }

        let niri = Niri {
            spacing: self.spacing,
            workspace_offset: self.workspace_offset,
            style: self.style.hydrate(colors),
            workspace_style: NiriWorkspaceStyle {
                active_hovered: workspace_active_hovered_style_merged,
                active: workspace_active_style_merged,
                hovered: workspace_hovered_style_merged,
                base: workspace_base_style,
            },
            window_style: NiriWindowStyle {
                focused: window_focused_style,
                base: window_base_style,
            },
            binds: self.binds.hydrate(),
        };

        ConfigModule::Niri(Box::new(niri))
    }
}

pub struct Niri {
    pub spacing: u32,
    pub workspace_offset: i8,
    pub style: ContainerStyle,
    pub workspace_style: NiriWorkspaceStyle,
    pub window_style: NiriWindowStyle,
    pub binds: MouseBinds,
}

pub struct NiriWorkspaceStyle {
    pub active_hovered: ContainerStyle,
    pub active: ContainerStyle,
    pub hovered: ContainerStyle,
    pub base: ContainerStyle,
}

pub struct NiriWindowStyle {
    pub focused: ContainerStyle,
    pub base: ContainerStyle,
}

#[derive(knuffel::Decode, Debug)]
pub struct RawLabel {
    #[knuffel(child, unwrap(argument), default = String::new())]
    pub text: String,

    #[knuffel(child, unwrap(argument), default = 18)]
    pub size: u32,

    #[knuffel(child, unwrap(argument), default = None)]
    pub tooltip: Option<String>,

    #[knuffel(flatten(child), default)]
    pub binds: RawMouseBinds,

    #[knuffel(child, default)]
    pub style: RawContainerStyle,
}

impl RawLabel {
    fn hydrate(self, colors: &ColorVars) -> ConfigModule {
        let label = Label {
            text: self.text,
            size: self.size,
            tooltip: self.tooltip,
            binds: self.binds.hydrate(),
            style: self.style.hydrate(colors),
        };

        ConfigModule::Label(label)
    }
}

pub struct Label {
    pub text: String,
    pub size: u32,
    pub tooltip: Option<String>,
    pub binds: MouseBinds,
    pub style: ContainerStyle,
}

#[derive(knuffel::Decode, Debug)]
pub struct RawSystemTray {}

impl RawSystemTray {
    fn hydrate(self, _colors: &ColorVars) -> ConfigModule {
        ConfigModule::SystemTray(SystemTray {})
    }
}

pub struct SystemTray {}

#[derive(knuffel::Decode, Debug, Clone, Default)]
pub struct RawMouseBinds {
    #[knuffel(child)]
    pub mouse_left: Option<Command>,

    #[knuffel(child)]
    pub double_click: Option<Command>,

    #[knuffel(child)]
    pub mouse_right: Option<Command>,

    #[knuffel(child)]
    pub mouse_middle: Option<Command>,

    #[knuffel(child)]
    pub scroll_up: Option<Command>,

    #[knuffel(child)]
    pub scroll_down: Option<Command>,

    #[knuffel(child)]
    pub scroll_right: Option<Command>,

    #[knuffel(child)]
    pub scroll_left: Option<Command>,
}

impl RawMouseBinds {
    fn hydrate(self) -> MouseBinds {
        fn process_command(cmd: Option<Command>) -> Option<Message> {
            let cmd = cmd?;
            if let Command::Normal(ref args) | Command::Sh(ref args) = cmd
                && args.is_empty()
            {
                return None;
            }
            match cmd {
                Command::Normal(args) => Some(Message::Command(CommandSpec {
                    command: args[0].clone(),
                    args: args.get(1..).map(<[String]>::to_vec),
                })),
                Command::Sh(args) => Some(Message::Command(CommandSpec {
                    command: String::from("sh"),
                    args: Some(vec![String::from("-c"), args[0].clone()]),
                })),
                Command::Niri(action) => Some(Message::Module(
                    ModuleMsg::Niri(NiriEvent::Action(action)),
                )),
                Command::None => None,
            }
        }

        MouseBinds {
            mouse_left: process_command(self.mouse_left),
            double_click: process_command(self.double_click),
            mouse_right: process_command(self.mouse_right),
            mouse_middle: process_command(self.mouse_middle),
            scroll: if self.scroll_up.is_some()
                || self.scroll_down.is_some()
                || self.scroll_left.is_some()
                || self.scroll_right.is_some()
            {
                Some(ScrollBinds {
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

#[derive(Debug, Clone, Default)]
pub struct MouseBinds {
    pub mouse_left: Option<Message>,
    pub double_click: Option<Message>,
    pub mouse_right: Option<Message>,
    pub mouse_middle: Option<Message>,
    pub scroll: Option<ScrollBinds>,
}

#[derive(Debug, Clone)]
pub struct ScrollBinds {
    pub up: Option<Message>,
    pub down: Option<Message>,
    pub right: Option<Message>,
    pub left: Option<Message>,
}

#[derive(Debug, Clone)]
pub enum RawMediaControl {
    Play,
    Pause,
    PlayPause,
    Stop,
    Next,
    Previous,
    Seek(i64),
    Volume(FloatOrInt<{ -i32::MAX }, { i32::MAX }>),
    SetVolume(FloatOrInt<0, { i32::MAX }>),
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
    Volume(f32),
    SetVolume(f32),
}

impl From<RawMediaControl> for MediaControl {
    fn from(value: RawMediaControl) -> Self {
        match value {
            RawMediaControl::Play => MediaControl::Play,
            RawMediaControl::Pause => MediaControl::Pause,
            RawMediaControl::PlayPause => MediaControl::PlayPause,
            RawMediaControl::Stop => MediaControl::Stop,
            RawMediaControl::Next => MediaControl::Next,
            RawMediaControl::Previous => MediaControl::Previous,
            RawMediaControl::Seek(x) => MediaControl::Seek(x),
            RawMediaControl::Volume(x) => MediaControl::Volume(x.into()),
            RawMediaControl::SetVolume(x) => MediaControl::SetVolume(x.into()),
        }
    }
}

impl<S> knuffel::Decode<S> for RawMediaControl
where
    S: knuffel::traits::ErrorSpan,
{
    fn decode_node(
        node: &knuffel::ast::SpannedNode<S>,
        ctx: &mut knuffel::decode::Context<S>,
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
                "play" => Ok(RawMediaControl::Play),
                "pause" => Ok(RawMediaControl::Pause),
                "play-pause" => Ok(RawMediaControl::PlayPause),
                "stop" => Ok(RawMediaControl::Stop),
                "next" => Ok(RawMediaControl::Next),
                "previous" => Ok(RawMediaControl::Previous),
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
                                Ok(seek_amount) => Ok(RawMediaControl::Seek(
                                    // convert from milliseconds to microseconds
                                    seek_amount * 1000,
                                )),
                                Err(e) => Err(DecodeError::unsupported(
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

                    match FloatOrInt::raw_decode(&second_arg.literal, ctx) {
                        Ok(amount) => Ok(RawMediaControl::Volume(amount)),
                        Err(e) => Err(e),
                    }
                }
                "set-volume" => {
                    let Some(second_arg) = iter_args.next() else {
                        return Err(DecodeError::missing(
                            node,
                            "set-volume requires additional argument",
                        ));
                    };

                    match FloatOrInt::raw_decode(&second_arg.literal, ctx) {
                        Ok(amount) => Ok(RawMediaControl::SetVolume(amount)),
                        Err(e) => Err(e),
                    }
                }
                _other => Err(DecodeError::unsupported(
                    &first_arg.literal,
                    "expected `play`, `pause`, `play-pause`, `stop`, `next`, `previous`, `seek`, `volume`, or `set-volume`",
                )),
            },
            _other => {
                Err(DecodeError::scalar_kind(Kind::String, &first_arg.literal))
            }
        }
    }
}

#[derive(knuffel::Decode, Debug, Clone, Default)]
pub struct RawMouseBindsForMpris {
    #[knuffel(child)]
    pub mouse_left: Option<RawMediaControl>,

    #[knuffel(child)]
    pub double_click: Option<RawMediaControl>,

    #[knuffel(child)]
    pub mouse_right: Option<RawMediaControl>,

    #[knuffel(child)]
    pub mouse_middle: Option<RawMediaControl>,

    #[knuffel(child)]
    pub scroll_up: Option<RawMediaControl>,

    #[knuffel(child)]
    pub scroll_down: Option<RawMediaControl>,

    #[knuffel(child)]
    pub scroll_right: Option<RawMediaControl>,

    #[knuffel(child)]
    pub scroll_left: Option<RawMediaControl>,
}

pub struct MouseBindsForMpris {
    pub mouse_left: Option<MediaControl>,
    pub double_click: Option<MediaControl>,
    pub mouse_right: Option<MediaControl>,
    pub mouse_middle: Option<MediaControl>,
    pub scroll_up: Option<MediaControl>,
    pub scroll_down: Option<MediaControl>,
    pub scroll_right: Option<MediaControl>,
    pub scroll_left: Option<MediaControl>,
}

impl From<RawMouseBindsForMpris> for MouseBindsForMpris {
    fn from(other: RawMouseBindsForMpris) -> Self {
        Self {
            mouse_left: other.mouse_left.map(Into::into),
            double_click: other.double_click.map(Into::into),
            mouse_right: other.mouse_right.map(Into::into),
            mouse_middle: other.mouse_middle.map(Into::into),
            scroll_up: other.scroll_up.map(Into::into),
            scroll_down: other.scroll_down.map(Into::into),
            scroll_right: other.scroll_right.map(Into::into),
            scroll_left: other.scroll_left.map(Into::into),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
enum CommandType {
    #[default]
    Normal,
    Sh,
    Niri,
}

#[derive(Debug, Clone, Default)]
pub enum Command {
    Normal(Vec<String>),
    Sh(Vec<String>),
    Niri(niri_ipc::Action),
    #[default]
    None,
}

#[derive(knuffel::Decode, Debug, Clone, Default)]
pub struct RawContainerStyle {
    #[knuffel(child, unwrap(argument))]
    pub text_color: Option<ConfigColor>,
    #[knuffel(child, unwrap(argument))]
    pub background: Option<ConfigColor>,
    #[knuffel(child)]
    pub border: Option<ConfigBorder>,
    #[knuffel(child, unwrap(argument))]
    pub padding: Option<FloatOrInt<0, { i32::MAX }>>,
}

impl RawContainerStyle {
    fn hydrate(self, color_vars: &ColorVars) -> ContainerStyle {
        ContainerStyle {
            inner: container::Style {
                text_color: self.text_color.map(|c| c.resolve(color_vars)),
                background: self
                    .background
                    .map(|c| c.resolve(color_vars))
                    .map(Background::Color),
                border: self.border.map_or(iced::Border::default(), |b| {
                    to_iced_border(b, color_vars)
                }),
                ..Default::default()
            },
            padding: self.padding.map(Into::into),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ContainerStyle {
    pub inner: container::Style,
    pub padding: Option<f32>,
}

impl Deref for ContainerStyle {
    type Target = container::Style;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for ContainerStyle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[derive(knuffel::Decode, Debug, Clone, Default)]
pub struct ConfigBorder {
    #[knuffel(child, unwrap(argument))]
    pub color: Option<ConfigColor>,
    #[knuffel(child, unwrap(argument), default)]
    pub width: Option<FloatOrInt<0, { i32::MAX }>>,
    #[knuffel(child)]
    pub radius: Option<RawConfigRadius>,
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
            .resolve(color_vars),
        width: border
            .width
            .unwrap_or(FloatOrInt(default_border.width))
            .into(),
        radius: match border.radius {
            Some(RawConfigRadius::All(r)) => iced::border::radius(r.into_f32()),
            Some(RawConfigRadius::PerCorner(PerCorner {
                top_left,
                top_right,
                bottom_left,
                bottom_right,
            })) => iced::border::Radius {
                top_left: top_left.into(),
                top_right: top_right.into(),
                bottom_right: bottom_right.into(),
                bottom_left: bottom_left.into(),
            },
            None => default_border.radius,
        },
    }
}

#[derive(Debug, Clone)]
pub enum RawConfigRadius {
    All(FloatOrInt<0, { i32::MAX }>),
    PerCorner(PerCorner),
}

impl From<RawConfigRadius> for iced::border::Radius {
    fn from(radius: RawConfigRadius) -> Self {
        match radius {
            RawConfigRadius::All(r) => iced::border::radius(r.into_f32()),
            RawConfigRadius::PerCorner(corners) => iced::border::Radius {
                top_left: corners.top_left.into(),
                top_right: corners.top_right.into(),
                bottom_left: corners.bottom_left.into(),
                bottom_right: corners.bottom_right.into(),
            },
        }
    }
}

#[derive(knuffel::Decode, Debug, Clone, Default)]
pub struct PerCorner {
    #[knuffel(child, unwrap(argument))]
    top_left: FloatOrInt<0, { i32::MAX }>,
    #[knuffel(child, unwrap(argument))]
    top_right: FloatOrInt<0, { i32::MAX }>,
    #[knuffel(child, unwrap(argument))]
    bottom_left: FloatOrInt<0, { i32::MAX }>,
    #[knuffel(child, unwrap(argument))]
    bottom_right: FloatOrInt<0, { i32::MAX }>,
}

impl<S> knuffel::Decode<S> for RawConfigRadius
where
    S: knuffel::traits::ErrorSpan,
{
    fn decode_node(
        node: &knuffel::ast::SpannedNode<S>,
        ctx: &mut knuffel::decode::Context<S>,
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
            Ok(Self::All(FloatOrInt::decode(radius, ctx)?))
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

impl<S> knuffel::Decode<S> for Command
where
    S: knuffel::traits::ErrorSpan,
{
    fn decode_node(
        node: &knuffel::ast::SpannedNode<S>,
        ctx: &mut knuffel::decode::Context<S>,
    ) -> Result<Self, DecodeError<S>> {
        if let Some(type_name) = &node.type_name {
            ctx.emit_error(DecodeError::unexpected(
                type_name,
                "type name",
                "no type name expected for this node",
            ));
        }

        let mut cmd_type = CommandType::default();
        for (name, val) in &node.properties {
            match &***name {
                "sh" => {
                    if let Some(true) =
                        knuffel::traits::DecodeScalar::decode(val, ctx)?
                    {
                        cmd_type = CommandType::Sh;
                    }
                }
                "niri" => {
                    if let Some(true) =
                        knuffel::traits::DecodeScalar::decode(val, ctx)?
                    {
                        cmd_type = CommandType::Niri;
                    }
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
        if iter_args.len() > 1 && cmd_type == CommandType::Sh {
            return Err(DecodeError::unexpected(
                &iter_args.nth(1).unwrap().literal,
                "argument",
                "when sh=true, only 1 argument is allowed",
            ));
        }
        if cmd_type == CommandType::Niri
            && let Some(arg) = node.arguments.first()
        {
            return Err(DecodeError::unexpected(
                &arg.literal,
                "argument",
                "when niri=true, no arguments are allowed",
            ));
        }

        let args = iter_args
            .map(|val| knuffel::traits::DecodeScalar::decode(val, ctx))
            .collect::<Result<_, _>>()?;

        let children = node.children();
        if matches!(cmd_type, CommandType::Normal | CommandType::Sh) {
            for unwanted_child in children {
                let name_str = &**unwanted_child.node_name;
                ctx.emit_error(DecodeError::unexpected(
                unwanted_child,
                "node",
                format!("unexpected node `{0}`", name_str.escape_default(),),
            ));
            }
        }

        match cmd_type {
            CommandType::Normal => Ok(Command::Normal(args)),
            CommandType::Sh => Ok(Command::Sh(args)),
            CommandType::Niri => {
                let mut children = node.children();
                if let Some(child) = children.next() {
                    for unwanted_child in children {
                        ctx.emit_error(DecodeError::unexpected(
                            unwanted_child,
                            "node",
                            "only one action is allowed when niri=true",
                        ));
                    }
                    let action =
                        niri_config::binds::Action::decode_node(child, ctx)?;
                    match config_to_ipc_action(action, child) {
                        Ok(action) => Ok(Command::Niri(action)),
                        Err(e) => {
                            ctx.emit_error(e);
                            Ok(Command::None)
                        }
                    }
                } else {
                    Err(DecodeError::missing(
                        node,
                        "expected a niri action for this bind",
                    ))
                }
            }
        }
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
        self.vars.get(name).copied()
    }

    pub fn parse(filename: &str, text: &str) -> miette::Result<Self> {
        match knuffel::parse::<ColorVars>(filename, text) {
            Ok(colors) => {
                debug!("Successfully parsed colors");
                Ok(colors)
            }
            Err(e) => Err(miette::Report::new(e)),
        }
    }
}

#[profiling::all_functions]
impl RawConfig {
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
        match knuffel::parse::<RawConfig>(filename, text) {
            Ok(config) => {
                debug!("Successfully parsed config");
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

        info!(
            "No config file detected, writing default config file to {}",
            path.display()
        );

        new_file
            .write_all(DEFAULT_CONFIG)
            .into_diagnostic()
            .with_context(|| {
                format!("error writing default config to {}", path.display())
            })?;

        Ok(())
    }

    pub fn load_or_create(path: &Path) -> miette::Result<Self> {
        RawConfig::create(path)?;
        RawConfig::load(path)
    }

    pub fn validate(config_dir: Option<PathBuf>) {
        let (config_path, colors_path) = get_config_paths(config_dir);

        print!("reading colors from \"{}\": ", colors_path.display());
        match ColorVars::load(&colors_path) {
            Err(e) => {
                println!("\n{e:?}");
            }
            Ok(_) => println!("{}", "valid".green()),
        }

        print!("reading config from \"{}\": ", config_path.display());
        match RawConfig::load(&config_path) {
            Err(e) => {
                println!("\n{e:?}");
            }
            Ok(_) => println!("{}", "valid".green()),
        }
    }

    pub fn init(
        config_dir: Option<PathBuf>,
    ) -> (Config, ColorVars, ConfigPath) {
        let (config_path, colors_path) = get_config_paths(config_dir);

        let colors = {
            match ColorVars::load(&colors_path) {
                Err(e) => {
                    debug!("Failed to parse colors file ");
                    debug!("{e:?}");
                    ColorVars::default()
                }
                Ok(colors) => colors,
            }
        };

        let raw_config = {
            match RawConfig::load_or_create(&config_path) {
                Err(e) => {
                    notification(
                        "Failed to parse config file, using default config\nrun `frostbar validate` to see the errors",
                    );
                    error!("Failed to parse config file, using default config");
                    error!("{e:?}");
                    RawConfig::default()
                }
                Ok(config) => config,
            }
        };

        let config = raw_config.hydrate(&colors);

        let path = ConfigPath {
            config: config_path,
            colors: colors_path,
        };

        (config, colors, path)
    }
}

fn get_config_paths(config_dir: Option<PathBuf>) -> (PathBuf, PathBuf) {
    let config_dir = config_dir.unwrap_or_else(|| {
        let home = if let Ok(xdg_config_home) = std::env::var("XDG_CONFIG_HOME")
        {
            PathBuf::from(xdg_config_home)
        } else {
            std::env::home_dir().unwrap()
        };
        home.join(".config").join("frostbar")
    });

    let config_path = config_dir.join("config.kdl");
    let colors_path = config_dir.join("colors.kdl");

    (config_path, colors_path)
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
                    notification(&format!(
                        "Color variable '{name}' not found, using red as default"
                    ));
                    Color::from_rgb(1.0, 0.0, 0.0)
                })
            }
        }
    }

    pub fn parse(&self) -> Color {
        match self {
            ConfigColor::Literal(c) => *c,
            ConfigColor::Variable(_) => {
                error!(
                    "Color variables not allowed in colors file, using red as default"
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

impl<S> knuffel::DecodeScalar<S> for ConfigColor
where
    S: knuffel::traits::ErrorSpan,
{
    fn type_check(
        type_name: &Option<knuffel::span::Spanned<knuffel::ast::TypeName, S>>,
        ctx: &mut knuffel::decode::Context<S>,
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
        value: &knuffel::span::Spanned<Literal, S>,
        _ctx: &mut knuffel::decode::Context<S>,
    ) -> Result<Self, DecodeError<S>> {
        match **value {
            knuffel::ast::Literal::String(ref s) => {
                if s.starts_with('$') {
                    Ok(ConfigColor::Variable(s.to_string()))
                } else {
                    let color = Color::from_str(s).map_err(|_| {
                        DecodeError::unsupported(value, "invalid hex literal, should be in the form \"#rrggbb[aa]\" or \"#rgb[a]\"")
                    })?;
                    Ok(ConfigColor::Literal(color))
                }
            }
            _ => Err(DecodeError::scalar_kind(Kind::String, value)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_default_config() {
        if let Err(e) =
            RawConfig::parse("", str::from_utf8(DEFAULT_CONFIG).unwrap())
        {
            panic!("{e}")
        }
    }
}
