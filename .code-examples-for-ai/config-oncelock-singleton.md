<!-- Demonstrates: OnceLock singleton for Config; serde JSON partial merge; validate_no_traversal security guard; platform-default resolution -->

```rust
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

// OnceLock<T> — immutable singleton; initialized at first config() call
static CONFIG: OnceLock<Config> = OnceLock::new();

/// Runtime configuration loaded from sjvm-conf.json, with safe platform defaults.
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub(crate) struct Config {
    /// Absolute path to the managed symlink (e.g. ~/.java/current)
    pub(crate) symlink_dir: String,
    /// Directories to scan for JDK installations
    pub(crate) jdks_dirs: Vec<String>,
}

/// Returns the global Config, loading and merging from disk on first call.
///
/// # Panics
/// Panics on startup if the config file exists but cannot be parsed.
pub(crate) fn config() -> &'static Config {
    CONFIG.get_or_init(|| init_config().expect("Failed to load configuration"))
}

fn init_config() -> anyhow::Result<Config> {
    let config_file = config_path();
    if config_file.is_file() {
        // Read raw bytes, parse as JSON Value, then merge with defaults
        let content = fs::read(&config_file)
            .with_context(|| format!("Cannot read config file '{}'", config_file.display()))?;
        let value: Value = serde_json::from_slice(&content).context("Cannot deserialize config")?;
        merge_config(value)
    } else {
        // First run — apply platform defaults
        let symlink_dir = default_symlink_dir()
            .ok_or_else(|| anyhow::anyhow!("Cannot determine platform data directory"))?;
        Ok(Config { symlink_dir, jdks_dirs: default_jdks_dirs() })
    }
}

/// Merges a partial JSON Value with platform defaults.
/// Only the fields present in the JSON override the defaults.
fn merge_config(config_value: Value) -> anyhow::Result<Config> {
    let symlink_dir = match &config_value["symlink_dir"] {
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
    // ... (jdks_dirs follows same pattern)
    Ok(Config { symlink_dir, jdks_dirs: default_jdks_dirs() })
}

/// Security guard: rejects paths with '..' components or NUL bytes.
/// Called on every config-supplied path before use.
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

/// Platform-specific path defaults — never hardcode paths.
fn default_symlink_dir() -> Option<String> {
    if cfg!(target_os = "windows") {
        Some("C:\\Java\\current".to_owned())
    } else {
        // Use `directories` crate for cross-platform home resolution
        UserDirs::new().map(|u| u.home_dir().join(".java/current").to_string_lossy().into_owned())
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

/// Returns the canonical path to the sjvm configuration file.
pub(crate) fn config_path() -> PathBuf {
    // app_dirs() uses the `directories` crate — no hardcoded paths
    Path::join(&app_dirs().config_dir, "sjvm-conf.json")
}

// --- Tests ------------------------------------------------------------------
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
    }

    #[test]
    fn test_merge_config_partial_json_uses_defaults_for_missing_fields() {
        let json = serde_json::json!({ "symlink_dir": "/custom/symlink" });
        let cfg = merge_config(json).unwrap();
        assert_eq!(cfg.symlink_dir, "/custom/symlink");
        assert!(!cfg.jdks_dirs.is_empty()); // filled with platform default
    }
}
```
