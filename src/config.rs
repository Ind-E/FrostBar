use std::{
    ffi::OsStr,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

use directories::ProjectDirs;
use miette::{Context, IntoDiagnostic};
use notify_rust::Notification;
use tracing::{debug, error};

use crate::constants::BAR_NAMESPACE;

#[derive(knuffel::Decode, Default, Debug)]
pub struct Config {
    #[knuffel(child, default)]
    pub layout: Layout,
    #[knuffel(child, default)]
    pub start: Start,
    #[knuffel(child, default)]
    pub middle: Middle,
    #[knuffel(child, default)]
    pub end: End,
}

#[derive(knuffel::Decode, Debug, Clone, Default)]
pub struct Start {
    #[knuffel(children, default)]
    pub modules: Vec<Module>,
}

#[derive(knuffel::Decode, Debug, Clone, Default)]
pub struct Middle {
    #[knuffel(children, default)]
    pub modules: Vec<Module>,
}

#[derive(knuffel::Decode, Debug, Clone, Default)]
pub struct End {
    #[knuffel(children, default)]
    pub modules: Vec<Module>,
}

#[derive(knuffel::Decode, Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    #[knuffel(child, unwrap(argument), default = 42)]
    pub width: u32,
    #[knuffel(child, unwrap(argument), default = 3)]
    pub gaps: i32,
    #[knuffel(child, unwrap(argument), default = 0)]
    pub border_radius: u16,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            width: 42,
            gaps: 3,
            border_radius: 0,
        }
    }
}

#[derive(knuffel::Decode, Debug, Clone)]
pub enum Module {
    Cava(Cava),
    Battery(Battery),
    Time(Time),
    Mpris(Mpris),
    Niri(Niri),
    Label(Label),
}

#[derive(knuffel::Decode, Debug, Clone)]
pub struct Cava {
    #[knuffel(child, unwrap(argument), default = 3)]
    pub volume_percent: i32,
    #[knuffel(child, unwrap(argument), default = 0.1)]
    pub spacing: f32,
    #[knuffel(flatten(child), default)]
    pub interaction: MouseInteraction,
}

#[derive(knuffel::Decode, Debug, Clone)]
pub struct Battery {
    #[knuffel(child, unwrap(argument), default = 22)]
    pub icon_size: u32,
    #[knuffel(child, unwrap(argument), default = 13)]
    pub overlay_icon_size: u32,
}

#[derive(knuffel::Decode, Debug, Clone)]
pub struct Time {
    #[knuffel(child, unwrap(argument), default = "%I\n%M".to_string())]
    pub format: String,
    #[knuffel(child, unwrap(argument), default = "%a %b %-d\n%-m/%-d/%y".to_string())]
    pub tooltip_format: String,
}

#[derive(knuffel::Decode, Debug, Clone)]
pub struct Mpris {
    #[knuffel(child, unwrap(argument), default = "󰝚".to_string())]
    pub placeholder: String,
}

#[derive(knuffel::Decode, Debug, Clone)]
pub struct Niri {
    #[knuffel(child, unwrap(argument), default = 10)]
    pub spacing: u32,
}

#[derive(knuffel::Decode, Debug, Clone)]
pub struct Label {
    #[knuffel(child, unwrap(argument), default = "".to_string())]
    pub text: String,
    #[knuffel(child, unwrap(argument), default = 18)]
    pub size: u32,
    #[knuffel(child, unwrap(argument), default = None)]
    pub tooltip: Option<String>,
    #[knuffel(flatten(child), default)]
    pub interaction: MouseInteraction,
}

#[derive(knuffel::Decode, Debug, Clone, Default)]
pub struct MouseInteraction {
    #[knuffel(child)]
    pub left_mouse: Option<Command>,
    #[knuffel(child)]
    pub right_mouse: Option<Command>,
    #[knuffel(child)]
    pub middle_mouse: Option<Command>,
    #[knuffel(child)]
    pub scroll_up: Option<Command>,
    #[knuffel(child)]
    pub scroll_down: Option<Command>,
}

#[derive(knuffel::Decode, Debug, Clone, Default)]
pub struct Command {
    #[knuffel(property)]
    pub sh: Option<String>,

    #[knuffel(arguments)]
    pub command: Option<Vec<String>>,
}

impl Config {
    pub fn load(path: &Path) -> miette::Result<Self> {
        let contents = fs::read_to_string(path)
            .into_diagnostic()
            .with_context(|| format!("error reading {path:?}"))?;

        let config = Self::parse(
            path.file_name()
                .and_then(OsStr::to_str)
                .unwrap_or("config.kdl"),
            &contents,
        )?;

        Ok(config)
    }

    pub fn parse(filename: &str, text: &str) -> miette::Result<Self> {
        match knuffel::parse::<Config>(filename, text) {
            Ok(config) => {
                debug!("Successfully parsed config");
                Ok(config)
            }
            Err(e) => {
                return Err(miette::Report::new(e));
            }
        }
    }

    pub fn create(path: &Path) -> miette::Result<()> {
        if let Some(default_parent) = path.parent() {
            fs::create_dir_all(default_parent)
                .into_diagnostic()
                .with_context(|| {
                    format!("error creating config directory {default_parent:?}")
                })?;
        }

        let mut new_file = match File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)
        {
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Ok(()),
            res => res,
        }
        .into_diagnostic()
        .with_context(|| format!("error opening config file at {path:?}"))?;

        let default_config = include_bytes!("../assets/default-config.kdl");

        new_file
            .write_all(default_config)
            .into_diagnostic()
            .with_context(|| format!("error writing default config to {path:?}"))?;

        Ok(())
    }

    pub fn load_or_create(path: &Path) -> miette::Result<Self> {
        Config::create(path)?;
        Config::load(path)
    }

    pub fn init() -> (Config, PathBuf) {
        let Some(project_dir) = ProjectDirs::from("", "", BAR_NAMESPACE) else {
            std::process::exit(1);
        };

        let config_path: PathBuf =
            project_dir.config_dir().to_path_buf().join("config.kdl");
        let config = {
            match Config::load_or_create(&config_path) {
                Err(e) => {
                    if let Err(e) = Notification::new()
                        .summary(BAR_NAMESPACE)
                        .body("Failed to parse config file, using default config")
                        .show()
                    {
                        error!("{e}");
                    };
                    eprintln!("\nFailed to parse config file, using default config");
                    eprintln!("{e:?}");
                    Config::default()
                }
                Ok(config) => config,
            }
        };

        (config, config_path)
    }
}
