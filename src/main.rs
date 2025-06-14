mod jdk_resolver;
mod list_command;
mod setup;
mod symlinks;
mod use_command;

use clap::{Parser, Subcommand};
use list_command::list_versions;
use setup::setup;
use use_command::use_version;

#[derive(Parser)]
#[command(name = "sjvm", version = "1.0", about = "Java version manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Setup,
    Use { version: String },
    List,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Setup => setup(),
        Commands::Use { version } => use_version(&version),
        Commands::List => list_versions(),
    }
}
