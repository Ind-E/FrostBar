use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(args_conflicts_with_subcommands = true)]
#[command(subcommand_value_name = "SUBCOMMAND")]
#[command(subcommand_help_heading = "Subcommands")]
pub struct Cli {
    /// Path to config directory (default: `$XDG_CONFIG_HOME/frostbar/`)
    ///
    /// Directory should contain a file named `config.kdl` and optionally a file named `colors.kdl`
    #[arg(short, long = "config", value_name = "FILE")]
    pub config_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub subcommand: Option<SubCommand>,
}

#[derive(Subcommand)]
pub enum SubCommand {
    /// Validate the config file
    Validate,
}
