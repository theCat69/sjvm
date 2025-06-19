use std::{
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use anyhow::Context;
use directories::UserDirs;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::app_dirs::app_dirs;

static CONFIG: OnceLock<Config> = OnceLock::new();

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Config {
    pub symlink_dir: String,
    pub jdks_dirs: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            symlink_dir: get_default_symlink_dir(),
            jdks_dirs: get_default_jdks_dirs(),
        }
    }
}

fn get_default_symlink_dir() -> String {
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
        panic!("OMG no home directories ? wtf dude")
    }
}

fn get_default_jdks_dirs() -> Vec<String> {
    
    if cfg!(target_os = "windows") {
        vec!["C:\\Program Files\\Java".to_string()]
    } else if cfg!(target_os = "macos") {
        vec!["/Library/Java/JavaVirtualMachines".to_string()]
    } else {
        vec!["/usr/lib/jvm".to_string()]
    }
}

pub fn config() -> &'static Config {
    CONFIG.get_or_init(|| init_config().unwrap())
}

fn init_config() -> Result<Config, anyhow::Error> {
    let config_file = get_config_path();
    if config_file.is_file() {
        let content = fs::read(config_file).with_context(|| "Cannot read config file")?;
        let config: Value =
            serde_json::from_slice(&content).with_context(|| "Cannot deserialize config")?;
        Ok(merge_config(config))
    } else {
        Ok(Config::default())
    }
}

fn merge_config(config_value: Value) -> Config {
    let symlink_dir_value = &config_value["symlink_dir"];
    let jdks_dirs_value = &config_value["jdks_dirs"];

    let symlink_dir = match symlink_dir_value {
        Value::Null => get_default_symlink_dir(),
        Value::String(string_value) => string_value.to_string(),
        _ => panic!("Symlink dir should be a string"),
    };
    let jdks_dirs = match jdks_dirs_value {
        Value::Null => get_default_jdks_dirs(),
        Value::Array(array_value) => array_value
            .iter()
            .map(|value| value.as_str().unwrap().to_string())
            .collect(),
        _ => panic!("Jdks dirs should be an array"),
    };

    Config {
        symlink_dir,
        jdks_dirs,
    }
}

pub fn get_config_path() -> PathBuf {
    Path::join(&app_dirs().config_dir, "sjvm-conf.json")
}
