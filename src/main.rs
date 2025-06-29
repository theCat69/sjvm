mod app_dirs;
mod config;
mod jdk_resolver;
mod list_command;
mod memory;
mod setup_command;
mod symlinks;
mod use_command;

use clap::{Parser, Subcommand};
use config::get_config_path;
use list_command::list_versions;
use setup_command::setup;
use use_command::{use_version, use_version_local};

#[derive(Parser)]
#[command(name = "sjvm", version = "1.0", about = "Java version manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Setup,
    Use {
        version: String,
        #[arg(short, long)]
        local: bool,
    },
    List,
    Config {
        #[command(subcommand)]
        config: Config,
    },
}

#[derive(Subcommand)]
enum Config {
    Path,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Setup => setup(),
        Commands::Use { version, local } => {
            if local {
                use_version_local(&version);
            } else {
                use_version(&version)
            }
        }
        Commands::List => list_versions(),
        Commands::Config { config } => match config {
            Config::Path => println!("{}", get_config_path().to_string_lossy()),
        },
    }
}
