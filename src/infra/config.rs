use std::{
    fs,
    path::{Component, Path, PathBuf},
    sync::OnceLock,
};

use anyhow::{Context, bail};
use directories::UserDirs;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::infra::app_dirs::app_dirs;

static CONFIG: OnceLock<Config> = OnceLock::new();

/// Dangerous system directories that configured paths must never resolve into.
#[cfg(unix)]
const DANGEROUS_PREFIXES: &[&str] = &["/etc", "/bin", "/sbin", "/usr/bin", "/usr/sbin"];

/// Runtime configuration loaded from `sjvm-conf.json` (or defaults if absent).
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub(crate) struct Config {
    /// Absolute path to the managed symlink that points to the active JDK.
    /// Defaults to `~/.java/current` on Unix or `C:\Java\current` on Windows.
    pub(crate) symlink_dir: String,
    /// List of directories to scan for JDK installations.
    /// Defaults to `/usr/lib/jvm` on Linux, `/Library/Java/JavaVirtualMachines`
    /// on macOS, or `C:\Program Files\Java` on Windows.
    pub(crate) jdks_dirs: Vec<String>,
}

fn default_symlink_dir() -> Option<String> {
    if cfg!(target_os = "windows") {
        Some("C:\\Java\\current".to_owned())
    } else {
        UserDirs::new().map(|user_dirs| {
            user_dirs
                .home_dir()
                .join(".java")
                .join("current")
                .to_string_lossy()
                .into_owned()
        })
    }
}

fn default_jdks_dirs() -> Vec<String> {
    if cfg!(target_os = "windows") {
        vec!["C:\\Program Files\\Java".to_owned()]
    } else if cfg!(target_os = "macos") {
        vec!["/Library/Java/JavaVirtualMachines".to_owned()]
    } else {
        vec!["/usr/lib/jvm".to_owned()]
    }
}

/// Returns the global `Config`, initialising it from disk on first call.
///
/// # Panics
/// Panics at startup if the config file exists but cannot be parsed; this is
/// intentional — a corrupted config is a fatal error.
pub(crate) fn config() -> &'static Config {
    CONFIG.get_or_init(|| init_config().expect("Failed to load configuration"))
}

fn init_config() -> anyhow::Result<Config> {
    let config_file = config_path();
    if config_file.is_file() {
        let content = fs::read(&config_file)
            .with_context(|| format!("Cannot read config file '{}'", config_file.display()))?;
        let value: Value = serde_json::from_slice(&content).context("Cannot deserialize config")?;
        merge_config(value)
    } else {
        let symlink_dir = default_symlink_dir()
            .ok_or_else(|| anyhow::anyhow!("Cannot determine platform data directory"))?;
        Ok(Config {
            symlink_dir,
            jdks_dirs: default_jdks_dirs(),
        })
    }
}

fn validate_no_traversal(p: &str, field: &str) -> anyhow::Result<()> {
    if p.contains('\0') {
        bail!("config field '{field}' contains a NUL byte which is not allowed");
    }
    let path = PathBuf::from(p);
    if path.components().any(|c| c == Component::ParentDir) {
        bail!("config field '{field}' contains path traversal ('..') which is not allowed");
    }
    Ok(())
}

/// Checks whether a canonicalized path starts with a dangerous system prefix.
/// Only performs the check if the path exists (canonicalization succeeded).
/// Emits a warning but does not fail hard — the dir may not exist on first run.
#[cfg(unix)]
fn warn_if_dangerous_path(raw: &str, field: &str) {
    let canonical = PathBuf::from(raw).canonicalize();
    if let Ok(canonical_path) = canonical {
        for prefix in DANGEROUS_PREFIXES {
            if canonical_path.starts_with(prefix) {
                eprintln!(
                    "sjvm: WARNING — config field '{}' resolves to a system directory ({}). \
                     This is almost certainly a misconfiguration.",
                    field,
                    canonical_path.display()
                );
            }
        }
    }
}

fn merge_config(config_value: Value) -> anyhow::Result<Config> {
    let symlink_dir_value = &config_value["symlink_dir"];
    let jdks_dirs_value = &config_value["jdks_dirs"];

    let symlink_dir = match symlink_dir_value {
        Value::Null => default_symlink_dir()
            .ok_or_else(|| anyhow::anyhow!("Cannot determine platform data directory"))?,
        Value::String(s) => {
            validate_no_traversal(s, "symlink_dir")?;
            #[cfg(unix)]
            warn_if_dangerous_path(s, "symlink_dir");
            s.clone()
        }
        _ => bail!("config field 'symlink_dir' must be a string"),
    };

    let jdks_dirs = match jdks_dirs_value {
        Value::Null => default_jdks_dirs(),
        Value::Array(arr) => {
            let mut dirs = Vec::with_capacity(arr.len());
            for (i, value) in arr.iter().enumerate() {
                let s = value
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("jdks_dirs[{i}] is not a string"))?;
                let field_name = format!("jdks_dirs[{i}]");
                validate_no_traversal(s, &field_name)?;
                #[cfg(unix)]
                warn_if_dangerous_path(s, &field_name);
                dirs.push(s.to_owned());
            }
            dirs
        }
        _ => bail!("config field 'jdks_dirs' must be an array"),
    };

    Ok(Config {
        symlink_dir,
        jdks_dirs,
    })
}

/// Returns the path to the sjvm configuration file.
///
/// The file is located in the platform-specific configuration directory
/// (e.g. `~/.config/sjvm/sjvm-conf.json` on Linux).
pub(crate) fn config_path() -> PathBuf {
    Path::join(&app_dirs().config_dir, "sjvm-conf.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_no_traversal_rejects_dotdot() {
        assert!(validate_no_traversal("/usr/../etc/passwd", "field").is_err());
        assert!(validate_no_traversal("../secret", "field").is_err());
    }

    #[test]
    fn test_validate_no_traversal_rejects_nul_byte() {
        assert!(validate_no_traversal("/usr/lib/jvm\0evil", "field").is_err());
    }

    #[test]
    fn test_validate_no_traversal_accepts_valid_paths() {
        assert!(validate_no_traversal("/usr/lib/jvm", "field").is_ok());
        assert!(validate_no_traversal("/home/user/.java/current", "field").is_ok());
        assert!(validate_no_traversal("relative/path", "field").is_ok());
    }

    #[test]
    fn test_merge_config_partial_json_uses_defaults_for_missing_fields() {
        let json = serde_json::json!({
            "symlink_dir": "/custom/symlink"
        });
        let cfg = merge_config(json).unwrap();
        assert_eq!(cfg.symlink_dir, "/custom/symlink");
        // jdks_dirs should be the platform default
        assert!(!cfg.jdks_dirs.is_empty());
    }

    #[test]
    fn test_merge_config_empty_json_uses_all_defaults() {
        let json = serde_json::json!({});
        let cfg = merge_config(json).unwrap();
        // Both fields should be filled with platform defaults — just verify non-empty.
        assert!(!cfg.symlink_dir.is_empty());
        assert!(!cfg.jdks_dirs.is_empty());
    }
}
