#![deny(unsafe_code)]
mod commands;
mod core;
mod infra;

use clap::{Parser, Subcommand};
use commands::list::list_versions;
use commands::setup::setup;
#[cfg(feature = "ui")]
use commands::ui::interactive_select;
use commands::use_cmd::{use_version, use_version_local};
use infra::config::config_path;

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
        /// Restrict to a specific vendor (skips custom JDKs with wrong vendor tag)
        #[arg(long, value_enum)]
        vendor: Option<core::jdk_catalog::Vendor>,
    },
    /// List available JDKs
    List,
    /// Download and install a JDK from Adoptium or GraalVM CE
    Install {
        /// JDK major version to install (8–25, e.g. "21")
        #[arg(value_name = "VERSION", value_parser = commands::install::validate_install_version)]
        version: String,
        /// JDK distribution vendor
        #[arg(long, value_enum, default_value_t = core::jdk_catalog::Vendor::OpenJdk)]
        vendor: core::jdk_catalog::Vendor,
        /// Target operating system (auto-detected if not specified)
        #[arg(long, value_name = "OS")]
        os: Option<String>,
        /// Target CPU architecture (auto-detected if not specified)
        #[arg(long, value_name = "ARCH")]
        arch: Option<String>,
        /// Overwrite an existing installation of the same JDK version
        #[arg(long)]
        force: bool,
        /// Path to a local .tar.gz archive to install directly (bypasses vendor API)
        #[arg(long, value_name = "PATH")]
        local_archive: Option<std::path::PathBuf>,
    },
    /// Remove an installed JDK (prompts for confirmation)
    Delete {
        /// Name of the JDK directory to remove
        #[arg(value_name = "NAME")]
        name: String,
    },
    /// List available JDK versions from vendor APIs
    Versions {
        /// JDK distribution vendor (shows both if omitted)
        #[arg(long, value_enum)]
        vendor: Option<core::jdk_catalog::Vendor>,
    },
    /// Tag an existing JDK with a vendor label
    Tag {
        /// Name of the JDK directory to tag
        #[arg(value_name = "NAME")]
        name: String,
        /// Vendor to assign
        #[arg(long, value_enum)]
        vendor: core::jdk_catalog::Vendor,
        /// Overwrite existing vendor tag
        #[arg(long)]
        force: bool,
    },
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
    commands::validate_version_string(s)?;
    Ok(s.to_owned())
}

#[cfg(test)]
mod tests {
    use clap::Parser as _;

    use super::{Cli, Commands, validate_version};

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

    #[test]
    fn test_install_command_parses_default_vendor() {
        let cli = Cli::try_parse_from(["sjvm", "install", "21"]).expect("should parse");
        if let Commands::Install {
            version,
            vendor,
            force,
            ..
        } = cli.command
        {
            assert_eq!(version, "21");
            assert_eq!(vendor, crate::core::jdk_catalog::Vendor::OpenJdk);
            assert!(!force);
        } else {
            panic!("expected Commands::Install");
        }
    }

    #[test]
    fn test_install_command_parses_graalvm_vendor() {
        let cli = Cli::try_parse_from(["sjvm", "install", "17", "--vendor", "graalvm"])
            .expect("should parse");
        if let Commands::Install { vendor, .. } = cli.command {
            assert_eq!(vendor, crate::core::jdk_catalog::Vendor::GraalVm);
        } else {
            panic!("expected Commands::Install");
        }
    }

    #[test]
    fn test_install_command_parses_force_flag() {
        let cli = Cli::try_parse_from(["sjvm", "install", "21", "--force"]).expect("should parse");
        if let Commands::Install { force, .. } = cli.command {
            assert!(force);
        } else {
            panic!("expected Commands::Install");
        }
    }

    #[test]
    fn test_install_command_rejects_unknown_vendor() {
        let result = Cli::try_parse_from(["sjvm", "install", "21", "--vendor", "zulu"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_install_command_rejects_version_out_of_range() {
        let result = Cli::try_parse_from(["sjvm", "install", "7"]);
        assert!(result.is_err(), "version 7 should be rejected");
    }

    #[test]
    fn test_delete_command_parses() {
        let cli = Cli::try_parse_from(["sjvm", "delete", "jdk-21"]).expect("should parse");
        if let Commands::Delete { name } = cli.command {
            assert_eq!(name, "jdk-21");
        } else {
            panic!("expected Commands::Delete");
        }
    }

    #[test]
    fn test_versions_command_parses_no_vendor() {
        let cli = Cli::try_parse_from(["sjvm", "versions"]).expect("should parse");
        if let Commands::Versions { vendor } = cli.command {
            assert!(vendor.is_none());
        } else {
            panic!("expected Commands::Versions");
        }
    }

    #[test]
    fn test_versions_command_parses_openjdk_vendor() {
        let cli =
            Cli::try_parse_from(["sjvm", "versions", "--vendor", "openjdk"]).expect("should parse");
        if let Commands::Versions { vendor } = cli.command {
            assert_eq!(vendor, Some(crate::core::jdk_catalog::Vendor::OpenJdk));
        } else {
            panic!("expected Commands::Versions");
        }
    }

    #[test]
    fn test_versions_command_parses_graalvm_vendor() {
        let cli =
            Cli::try_parse_from(["sjvm", "versions", "--vendor", "graalvm"]).expect("should parse");
        if let Commands::Versions { vendor } = cli.command {
            assert_eq!(vendor, Some(crate::core::jdk_catalog::Vendor::GraalVm));
        } else {
            panic!("expected Commands::Versions");
        }
    }

    #[test]
    fn test_use_command_parses_vendor() {
        let cli = Cli::try_parse_from(["sjvm", "use", "17", "--vendor", "graalvm"])
            .expect("should parse");
        if let Commands::Use {
            version, vendor, ..
        } = cli.command
        {
            assert_eq!(version, "17");
            assert_eq!(vendor, Some(crate::core::jdk_catalog::Vendor::GraalVm));
        } else {
            panic!("expected Commands::Use");
        }
    }

    #[test]
    fn test_tag_command_parses() {
        let cli = Cli::try_parse_from(["sjvm", "tag", "jdk-17", "--vendor", "openjdk"])
            .expect("should parse");
        if let Commands::Tag {
            name,
            vendor,
            force,
        } = cli.command
        {
            assert_eq!(name, "jdk-17");
            assert_eq!(vendor, crate::core::jdk_catalog::Vendor::OpenJdk);
            assert!(!force);
        } else {
            panic!("expected Commands::Tag");
        }
    }

    #[test]
    fn test_tag_command_parses_force() {
        let cli = Cli::try_parse_from(["sjvm", "tag", "jdk-17", "--vendor", "openjdk", "--force"])
            .expect("should parse");
        if let Commands::Tag { force, .. } = cli.command {
            assert!(force);
        } else {
            panic!("expected Commands::Tag");
        }
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
        Commands::Use {
            version,
            local,
            vendor,
        } => {
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
        Commands::List => {
            if let Err(e) = list_versions() {
                eprintln!("❌ List failed: {e}");
                std::process::exit(1);
            }
        }
        Commands::Install {
            version,
            vendor,
            os,
            arch,
            force,
            local_archive,
        } => {
            if let Err(e) = commands::install::run_install(
                &version,
                &vendor,
                os.as_deref(),
                arch.as_deref(),
                force,
                local_archive,
            ) {
                eprintln!("❌ Install failed: {e}");
                std::process::exit(1);
            }
        }
        Commands::Delete { name } => {
            if let Err(e) = commands::delete::run_delete(&name) {
                eprintln!("❌ Delete failed: {e}");
                std::process::exit(1);
            }
        }
        Commands::Versions { vendor } => {
            if let Err(e) = commands::versions::run_versions(vendor.as_ref()) {
                eprintln!("❌ Versions failed: {e}");
                std::process::exit(1);
            }
        }
        Commands::Tag {
            name,
            vendor,
            force,
        } => {
            if let Err(e) = commands::tag::run_tag(&name, &vendor, force) {
                eprintln!("❌ Tag failed: {e}");
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
