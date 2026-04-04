<!-- Demonstrates: clap derive API — Parser/Subcommand structs, value_parser, ValueEnum, try_parse_from in tests -->

```rust
// Crate root attr — deny all unsafe code
#![deny(unsafe_code)]

use clap::{Parser, Subcommand};

/// Java version manager via symlinks
// #[command(version)] auto-populates from Cargo.toml — never hardcode
#[derive(Parser)]
#[command(name = "sjvm", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Switch the active JDK
    Use {
        /// Version string to match (e.g. "17", "temurin-21", "graalvm")
        // value_parser hooks a custom validator; value_name sets the metavar in --help
        #[arg(value_name = "VERSION", value_parser = validate_version)]
        version: String,

        /// Print shell export commands instead of switching globally
        #[arg(short, long)]
        local: bool,

        /// Restrict to a specific vendor
        // ValueEnum = closed set; clap rejects unknown strings automatically
        #[arg(long, value_enum)]
        vendor: Option<core::jdk_catalog::Vendor>,
    },

    /// Download and install a JDK from Adoptium or GraalVM CE
    Install {
        /// JDK major version to install (8–25, e.g. "21")
        #[arg(value_name = "VERSION", value_parser = commands::install::validate_install_version)]
        version: String,

        /// Overwrite an existing installation
        #[arg(long)]
        force: bool,
    },

    // Feature-gated subcommand — only compiled when the `ui` feature is enabled
    #[cfg(feature = "ui")]
    /// Interactive TUI for selecting a JDK
    Ui,
}

fn validate_version(s: &str) -> Result<String, String> {
    commands::validate_version_string(s)?;
    Ok(s.to_owned())
}

fn main() {
    // Cli::parse() exits on bad input — correct for main(); never use in tests
    let cli = Cli::parse();
    match cli.command {
        Commands::Use { version, local, vendor } => {
            // delegate immediately — no business logic in main
            let result = if local {
                use_version_local(&version, vendor.as_ref())
            } else {
                use_version(&version, vendor.as_ref())
            };
            if let Err(e) = result {
                eprintln!("❌ {e}");
                std::process::exit(1);
            }
        }
        Commands::Install { version, force } => {
            if let Err(e) = commands::install::run_install(&version, force) {
                eprintln!("❌ Install failed: {e}");
                std::process::exit(1);
            }
        }
        #[cfg(feature = "ui")]
        Commands::Ui => { /* ... */ }
    }
}

// --- Tests ------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use clap::Parser as _;
    use super::{Cli, Commands};

    // ALWAYS use try_parse_from in tests — Cli::parse() calls process::exit on failure
    #[test]
    fn test_install_command_parses_default_vendor() {
        let cli = Cli::try_parse_from(["sjvm", "install", "21"]).expect("should parse");
        if let Commands::Install { version, force, .. } = cli.command {
            assert_eq!(version, "21");
            assert!(!force);
        } else {
            panic!("expected Commands::Install");
        }
    }

    #[test]
    fn test_install_command_rejects_unknown_vendor() {
        // try_parse_from returns Err instead of exiting
        let result = Cli::try_parse_from(["sjvm", "install", "21", "--vendor", "zulu"]);
        assert!(result.is_err());
    }
}
```
