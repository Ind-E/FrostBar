use std::{
    ffi::OsStr,
    fs::{self, File},
    io::Write,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
};

use directories::ProjectDirs;
use iced::{Color, color};
use knus::{DecodeScalar, errors::DecodeError};
use miette::{Context, IntoDiagnostic};
use notify_rust::Notification;
use tracing::{error, info};

use crate::constants::BAR_NAMESPACE;

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

#[derive(knus::Decode, Debug, Default)]
pub struct Start {
    #[knus(children, default)]
    pub modules: Vec<Module>,
}

#[derive(knus::Decode, Debug, Default)]
pub struct Middle {
    #[knus(children, default)]
    pub modules: Vec<Module>,
}

#[derive(knus::Decode, Debug, Default)]
pub struct End {
    #[knus(children, default)]
    pub modules: Vec<Module>,
}

#[derive(knus::Decode, Debug, Clone, PartialEq)]
pub struct Layout {
    #[knus(child, unwrap(argument), default = 42)]
    pub width: u32,
    #[knus(child, unwrap(argument), default = 0)]
    pub gaps: i32,
    #[knus(child, unwrap(argument), default = Self::default().anchor)]
    pub anchor: Anchor,
}

#[derive(knus::DecodeScalar, Debug, Clone, PartialEq)]
pub enum Anchor {
    Left,
    Right,
    Top,
    Bottom,
}

impl Anchor {
    pub fn vertical(&self) -> bool {
        match self {
            Anchor::Left | Anchor::Right => true,
            Anchor::Top | Anchor::Bottom => false,
        }
    }
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            width: 42,
            gaps: 3,
            anchor: Anchor::Left,
        }
    }
}

#[derive(knus::Decode, Debug, Clone)]
pub struct Style {
    #[knus(child, unwrap(argument), default = 0)]
    pub border_radius: u16,
    #[knus(child, default = Self::default().background)]
    pub background: ConfigColor,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            border_radius: 0,
            background: Color::from_rgb(0.0, 0.0, 0.0).into(),
        }
    }
}

#[derive(knus::Decode, Debug)]
pub enum Module {
    Cava(Cava),
    Battery(Battery),
    Time(Time),
    Mpris(Mpris),
    Niri(Niri),
    Label(Label),
}

#[derive(knus::Decode, Debug)]
pub struct Cava {
    #[knus(child, unwrap(argument), default = 3)]
    pub volume_percent: i32,

    #[knus(child, unwrap(argument), default = 0.1)]
    pub spacing: f32,

    #[knus(flatten(child), default)]
    pub binds: MouseBinds,

    #[knus(child, default)]
    pub style: ContainerStyle,
}

#[derive(knus::Decode, Debug)]
pub struct Battery {
    #[knus(child, unwrap(argument), default = Self::default().icon_size)]
    pub icon_size: u32,

    #[knus(child, default = Self::default().charging_color)]
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

#[derive(knus::Decode, Debug, Clone)]
pub struct Mpris {
    #[knus(child, unwrap(argument), default = "󰝚".to_string())]
    pub placeholder: String,

    #[knus(flatten(child), default)]
    pub binds: MouseBindsForMpris,

    #[knus(child, default)]
    pub placeholder_style: ContainerStyle,
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
    pub window_style: ContainerStyle,

    #[knus(child, default)]
    pub workspace_active_style: ContainerStyle,

    #[knus(child, default)]
    pub workspace_style: ContainerStyle,
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

#[derive(knus::Decode, Debug, Clone, Default)]
pub struct MouseBinds {
    #[knus(child)]
    pub mouse_left: Option<Command>,

    #[knus(child)]
    pub mouse_right: Option<Command>,

    #[knus(child)]
    pub mouse_middle: Option<Command>,

    #[knus(child)]
    pub scroll_up: Option<Command>,

    #[knus(child)]
    pub scroll_down: Option<Command>,
}

#[derive(knus::DecodeScalar, Debug, Clone, Copy, PartialEq)]
pub enum MediaControl {
    Play,
    Pause,
    PlayPause,
    Stop,
    Next,
    Previous,
}

#[derive(knus::Decode, Debug, Clone, Default)]
pub struct MouseBindsForMpris {
    #[knus(child, unwrap(argument))]
    pub mouse_left: Option<MediaControl>,

    #[knus(child, unwrap(argument))]
    pub mouse_right: Option<MediaControl>,

    #[knus(child, unwrap(argument))]
    pub mouse_middle: Option<MediaControl>,

    #[knus(child, unwrap(argument))]
    pub scroll_up: Option<MediaControl>,

    #[knus(child, unwrap(argument))]
    pub scroll_down: Option<MediaControl>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Command {
    pub sh: Option<bool>,

    pub args: Vec<String>,
}

#[derive(knus::Decode, Debug, Clone, Default)]
pub struct ContainerStyle {
    #[knus(child)]
    pub text_color: Option<ConfigColor>,
    #[knus(child)]
    pub background: Option<ConfigColor>,
    #[knus(child)]
    pub border: Option<ConfigBorder>,
}

#[derive(knus::Decode, Debug, Clone)]
pub struct ConfigBorder {
    #[knus(child, default = Self::default().color)]
    pub color: ConfigColor,
    #[knus(child, unwrap(argument), default = Self::default().width)]
    pub width: f32,
    #[knus(child, default = Self::default().radius)]
    pub radius: ConfigRadius,
}

impl Default for ConfigBorder {
    fn default() -> Self {
        Self {
            color: ConfigColor {
                inner: iced::Color::WHITE,
            },
            width: 0.0,
            radius: ConfigRadius::All(0.0),
        }
    }
}

impl From<ConfigBorder> for iced::Border {
    fn from(border: ConfigBorder) -> Self {
        iced::Border {
            color: border.color.into(),
            width: border.width,
            radius: match border.radius {
                ConfigRadius::All(r) => iced::border::radius(r),
                ConfigRadius::PerCorner(PerCorner {
                    top_left,
                    top_right,
                    bottom_left,
                    bottom_right,
                }) => iced::border::Radius {
                    top_left,
                    top_right,
                    bottom_right,
                    bottom_left,
                },
            },
        }
    }
}

#[derive(Debug, Clone)]
pub enum ConfigRadius {
    All(f32),
    PerCorner(PerCorner),
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

        for property in node.properties.iter() {
            ctx.emit_error(DecodeError::unexpected(
                property.0,
                "property",
                "no propertes expected for this node",
            ));
        }

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
            for child in node.children.iter() {
                ctx.emit_error(DecodeError::unexpected(
                    &child,
                    "node",
                    "no children expected when radius is specified as an argument",
                ));
            }
            let radius = iter_args.next().unwrap();
            Ok(Self::All(f32::decode(radius, ctx)?))
        } else {
            for argument in iter_args {
                ctx.emit_error(DecodeError::unexpected(
                    &argument.literal,
                    "argument",
                    "expected 1 or 0 arguments",
                ));
            }

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

    pub fn init() -> (Config, PathBuf, PathBuf) {
        let Some(project_dir) = ProjectDirs::from("", "", BAR_NAMESPACE) else {
            std::process::exit(1);
        };

        let config_dir = project_dir.config_dir().to_path_buf();

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
                    eprintln!(
                        "\nFailed to parse config file, using default config"
                    );
                    eprintln!("{e:?}");
                    Config::default()
                }
                Ok(config) => config,
            }
        };

        (config, config_path, config_dir)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigColor {
    inner: Color,
}

impl Deref for ConfigColor {
    type Target = Color;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for ConfigColor {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl From<&ConfigColor> for Color {
    fn from(color: &ConfigColor) -> Self {
        color.inner
    }
}

impl From<ConfigColor> for Color {
    fn from(color: ConfigColor) -> Self {
        color.inner
    }
}

impl From<Color> for ConfigColor {
    fn from(inner: Color) -> Self {
        Self { inner }
    }
}

#[derive(knus::Decode)]
struct ColorRgba {
    #[knus(argument)]
    r: u8,
    #[knus(argument)]
    g: u8,
    #[knus(argument)]
    b: u8,
    #[knus(argument)]
    a: Option<f32>,
}

impl<S> knus::Decode<S> for ConfigColor
where
    S: knus::traits::ErrorSpan,
{
    fn decode_node(
        node: &knus::ast::SpannedNode<S>,
        ctx: &mut knus::decode::Context<S>,
    ) -> Result<Self, DecodeError<S>> {
        // Check for unexpected type name.
        if let Some(type_name) = &node.type_name {
            ctx.emit_error(DecodeError::unexpected(
                type_name,
                "type name",
                "no type name expected for this node",
            ));
        }

        // Get the first argument.
        let mut iter_args = node.arguments.iter();
        let val = iter_args.next().ok_or_else(|| {
            DecodeError::missing(node, "additional argument is required")
        })?;

        // Check for unexpected type name.
        if let Some(typ) = &val.type_name {
            ctx.emit_error(DecodeError::TypeName {
                span: typ.span().clone(),
                found: Some((**typ).clone()),
                expected: knus::errors::ExpectedType::no_type(),
                rust_type: "str",
            });
        }

        // Check the argument type.
        let rv = match *val.literal {
            // If it's a string, use parse.
            knus::ast::Literal::String(ref s) => {
                Color::parse(s).ok_or_else(|| {
                    DecodeError::conversion(&val.literal, "invalid hex literal")
                })
            }
            // Otherwise, fall back to the 4-argument RGBA form.
            _ => {
                return ColorRgba::decode_node(node, ctx).map(
                    |ColorRgba { r, g, b, a }| {
                        if let Some(a) = a {
                            Color::from_rgba8(r, g, b, a).into()
                        } else {
                            Color::from_rgb8(r, g, b).into()
                        }
                    },
                );
            }
        }?;

        // Check for unexpected following arguments.
        if let Some(val) = iter_args.next() {
            ctx.emit_error(DecodeError::unexpected(
                &val.literal,
                "argument",
                "unexpected argument",
            ));
        }

        // Check for unexpected properties and children.
        for name in node.properties.keys() {
            ctx.emit_error(DecodeError::unexpected(
                name,
                "property",
                format!("unexpected property `{}`", name.escape_default()),
            ));
        }
        for child in node.children.as_ref().map_or(&[][..], |lst| &lst[..]) {
            ctx.emit_error(DecodeError::unexpected(
                child,
                "node",
                format!(
                    "unexpected node `{}`",
                    child.node_name.escape_default()
                ),
            ));
        }

        Ok(rv.into())
    }
}
