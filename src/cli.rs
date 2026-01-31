use std::path::PathBuf;
use std::process::exit;

use clap::{Parser, Subcommand};

use crate::{config::RawConfig, utils::log::LogManager};

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(args_conflicts_with_subcommands = true)]
#[command(subcommand_value_name = "SUBCOMMAND")]
#[command(subcommand_help_heading = "Subcommands")]
pub struct Cli {
    /// Path to config directory (default: `$XDG_CONFIG_HOME/frostbar/`)
    ///
    /// Directory should contain a file named `config.kdl` and optionally a file named `colors.kdl`
    #[arg(short, long = "config", value_name = "DIRECTORY")]
    pub config_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub subcommand: Option<SubCommand>,
}

#[derive(Subcommand)]
pub enum SubCommand {
    /// Validate the config file
    Validate {
        #[arg(short, long)]
        config_dir: Option<PathBuf>,
    },
    Logs {
        #[arg(short, long)]
        pid: Option<u32>,
    },
}

#[derive(Subcommand, Default)]
pub enum LogSubCommand {
    Pid {
        pid: u32,
    },
    #[default]
    Latest,
}

pub fn handle_subcommand(sub: SubCommand, log_manager: &LogManager) {
    match sub {
        SubCommand::Validate { config_dir } => {
            RawConfig::validate(config_dir);
        }
        SubCommand::Logs { pid } => {
            if let Some(ref path) = log_manager.find_log(pid) {
                if let Err(_) = std::process::Command::new("less")
                    .arg("+G") // jump to end
                    .arg("-RX") // color, don't clear screen on exit
                    .arg(path)
                    .status()
                {
                    println!("{}", path.display());
                }
            } else {
                println!("no log files found");
            }
        }
    }
    exit(0);
}
