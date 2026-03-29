#![deny(unsafe_code)]
mod app_dirs;
mod config;
mod jdk_resolver;
mod jdk_switcher;
mod list_command;
mod memory;
mod setup_command;
mod symlinks;
#[cfg(feature = "ui")]
mod ui_command;
mod use_command;

use clap::{Parser, Subcommand};
use config::config_path;
use list_command::list_versions;
use setup_command::setup;
#[cfg(feature = "ui")]
use ui_command::interactive_select;
use use_command::{use_version, use_version_local};

/// Java version manager via symlinks
#[derive(Parser)]
#[command(name = "sjvm", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// First-run setup: creates the initial JDK symlink
    Setup,
    /// Switch the active JDK
    Use {
        /// Version string to match (e.g. "17", "temurin-21", "graalvm")
        #[arg(value_name = "VERSION", value_parser = validate_version)]
        version: String,
        /// Print shell export commands instead of switching globally
        #[arg(short, long)]
        local: bool,
    },
    /// List available JDKs
    List,
    #[cfg(feature = "ui")]
    /// Interactive TUI for selecting a JDK
    Ui,
    /// Configuration utilities
    Config {
        #[command(subcommand)]
        config: ConfigCmd,
    },
}

#[derive(Subcommand)]
enum ConfigCmd {
    /// Print the path to the configuration file
    Path,
}

fn validate_version(s: &str) -> Result<String, String> {
    if s.is_empty() {
        return Err("version cannot be empty".to_owned());
    }
    if s.len() > 64 {
        return Err("version string too long (max 64 chars)".to_owned());
    }
    if !s.chars().all(|c| c.is_alphanumeric() || "-._".contains(c)) {
        return Err(
            "version contains illegal characters (only alphanumeric, '-', '.', '_' allowed)"
                .to_owned(),
        );
    }
    Ok(s.to_owned())
}

#[cfg(test)]
mod tests {
    use super::validate_version;

    #[test]
    fn test_validate_version_rejects_empty() {
        assert!(validate_version("").is_err());
    }

    #[test]
    fn test_validate_version_rejects_too_long() {
        let long = "a".repeat(65);
        assert!(validate_version(&long).is_err());
    }

    #[test]
    fn test_validate_version_rejects_metacharacters() {
        for bad in &["17;rm", "17$HOME", "17`id`", "17|cat", "17>out", "17("] {
            assert!(
                validate_version(bad).is_err(),
                "expected error for version '{bad}'"
            );
        }
    }

    #[test]
    fn test_validate_version_accepts_valid_strings() {
        for good in &[
            "17",
            "temurin-21",
            "graalvm-ce-java17",
            "1.8.0_391",
            "zulu-8",
        ] {
            assert!(
                validate_version(good).is_ok(),
                "expected ok for version '{good}'"
            );
        }
    }

    #[test]
    fn test_validate_version_returns_owned_input() {
        let result = validate_version("17").unwrap();
        assert_eq!(result, "17");
    }

    #[test]
    fn test_validate_version_accepts_max_length() {
        let exactly_64 = "a".repeat(64);
        assert!(validate_version(&exactly_64).is_ok());
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Setup => {
            if let Err(e) = setup() {
                eprintln!("❌ Setup failed: {e}");
                std::process::exit(1);
            }
        }
        Commands::Use { version, local } => {
            let result = if local {
                use_version_local(&version)
            } else {
                use_version(&version)
            };
            if let Err(e) = result {
                eprintln!("❌ {e}");
                std::process::exit(1);
            }
        }
        Commands::List => {
            if let Err(e) = list_versions() {
                eprintln!("❌ List failed: {e}");
                std::process::exit(1);
            }
        }
        #[cfg(feature = "ui")]
        Commands::Ui => {
            if let Err(e) = interactive_select() {
                eprintln!("❌ UI failed: {e}");
                std::process::exit(1);
            }
        }
        Commands::Config { config } => match config {
            ConfigCmd::Path => println!("{}", config_path().to_string_lossy()),
        },
    }
}
