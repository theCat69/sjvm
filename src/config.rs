use std::{
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use serde::{Deserialize, Serialize};

use crate::app_dirs::app_dirs;

static CONFIG: OnceLock<Config> = OnceLock::new();

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Config {
    pub setup_dirs: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            setup_dirs: get_default_setup_dirs(),
        }
    }
}

fn get_default_setup_dirs() -> Vec<String> {
    let candidates = if cfg!(target_os = "windows") {
        vec!["C:\\Program Files\\Java".to_string()]
    } else if cfg!(target_os = "macos") {
        vec!["/Library/Java/JavaVirtualMachines".to_string()]
    } else {
        vec!["/usr/lib/jvm".to_string()]
    };
    candidates
}

pub fn config() -> &'static Config {
    CONFIG.get_or_init(|| init_config())
}

fn init_config() -> Config {
    let config_file = get_config_path();
    println!("config_file {}", config_file.to_string_lossy());
    if config_file.is_file() {
        let content = fs::read(config_file).unwrap();
        serde_json::from_slice(&content).unwrap()
    } else {
        Config::default()
    }
}

pub fn get_config_path() -> PathBuf {
    Path::join(&app_dirs().config_dir, "sjvm-conf.json")
}
