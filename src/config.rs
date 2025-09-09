use std::{
    ffi::OsStr,
    fs::{self, File},
    io::Write,
    path::Path,
};

use miette::{Context, IntoDiagnostic};

#[derive(knuffel::Decode, Default, Debug)]
pub struct Config {
    #[knuffel(child, default)]
    pub layout: Layout,
    #[knuffel(child, default)]
    pub modules: Modules,
}

#[derive(knuffel::Decode, Debug, Clone)]
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

#[derive(knuffel::Decode, Default, Debug)]
pub struct Modules {
    #[knuffel(child, default)]
    pub cava: Cava,
    #[knuffel(child, default)]
    pub battery: Battery,
}

#[derive(knuffel::Decode, Debug, Clone)]
pub struct Cava {
    #[knuffel(child, unwrap(argument), default = 3)]
    pub volume_percent: i32,
    #[knuffel(child, unwrap(argument), default = 0.1)]
    pub spacing: f32,
}

impl Default for Cava {
    fn default() -> Self {
        Self {
            volume_percent: 3,
            spacing: 0.1,
        }
    }
}

#[derive(knuffel::Decode, Debug, Clone)]
pub struct Battery {
    #[knuffel(child, unwrap(argument), default = 22)]
    pub icon_size: u32,
    #[knuffel(child, unwrap(argument), default = 13)]
    pub overlay_icon_size: u32,
}

impl Default for Battery {
    fn default() -> Self {
        Self {
            icon_size: 22,
            overlay_icon_size: 13,
        }
    }
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
        )
        .context("error parsing config")?;

        Ok(config)
    }

    pub fn parse(filename: &str, text: &str) -> miette::Result<Self> {
        match knuffel::parse::<Config>(filename, text) {
            Ok(config) => Ok(config),
            Err(e) => {
                println!("{:?}", miette::Report::new(e));
                std::process::exit(1);
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
}
