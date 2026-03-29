use std::{
    fs,
    path::{Component, Path, PathBuf},
    sync::OnceLock,
};

use anyhow::{bail, Context};
use directories::UserDirs;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::app_dirs::app_dirs;

static CONFIG: OnceLock<Config> = OnceLock::new();

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub(crate) struct Config {
    pub(crate) symlink_dir: String,
    pub(crate) jdks_dirs: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            symlink_dir: default_symlink_dir(),
            jdks_dirs: default_jdks_dirs(),
        }
    }
}

fn default_symlink_dir() -> String {
    if cfg!(target_os = "windows") {
        "C:\\Java\\current".to_string()
    } else if let Some(user_dirs) = UserDirs::new() {
        user_dirs
            .home_dir()
            .join(".java")
            .join("current")
            .to_string_lossy()
            .into_owned()
    } else {
        // Fallback: best-effort path; will surface as an error later if needed
        String::from("/tmp/.java/current")
    }
}

fn default_jdks_dirs() -> Vec<String> {
    if cfg!(target_os = "windows") {
        vec!["C:\\Program Files\\Java".to_string()]
    } else if cfg!(target_os = "macos") {
        vec!["/Library/Java/JavaVirtualMachines".to_string()]
    } else {
        vec!["/usr/lib/jvm".to_string()]
    }
}

/// Returns the global `Config`, initialising it from disk on first call.
///
/// # Errors
/// Panics at startup (via `.expect`) if the config file exists but cannot be
/// parsed; this is intentional — a corrupted config is a fatal error.
pub(crate) fn config() -> &'static Config {
    CONFIG.get_or_init(|| init_config().expect("Failed to load configuration"))
}

fn init_config() -> anyhow::Result<Config> {
    let config_file = config_path();
    if config_file.is_file() {
        let content = fs::read(&config_file)
            .with_context(|| format!("Cannot read config file '{}'", config_file.display()))?;
        let value: Value =
            serde_json::from_slice(&content).with_context(|| "Cannot deserialize config")?;
        merge_config(value)
    } else {
        Ok(Config::default())
    }
}

fn validate_no_traversal(p: &str, field: &str) -> anyhow::Result<()> {
    if p.contains('\0') {
        bail!(
            "config field '{}' contains a NUL byte which is not allowed",
            field
        );
    }
    let path = PathBuf::from(p);
    if path.components().any(|c| c == Component::ParentDir) {
        bail!(
            "config field '{}' contains path traversal ('..') which is not allowed",
            field
        );
    }
    Ok(())
}

fn merge_config(config_value: Value) -> anyhow::Result<Config> {
    let symlink_dir_value = &config_value["symlink_dir"];
    let jdks_dirs_value = &config_value["jdks_dirs"];

    let symlink_dir = match symlink_dir_value {
        Value::Null => default_symlink_dir(),
        Value::String(s) => {
            validate_no_traversal(s, "symlink_dir")?;
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
                    .ok_or_else(|| anyhow::anyhow!("jdks_dirs[{}] is not a string", i))?;
                validate_no_traversal(s, &format!("jdks_dirs[{}]", i))?;
                dirs.push(s.to_string());
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
pub(crate) fn config_path() -> PathBuf {
    Path::join(&app_dirs().config_dir, "sjvm-conf.json")
}
