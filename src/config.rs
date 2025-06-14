use std::{path::PathBuf, sync::OnceLock};

static CONFIG: OnceLock<Config> = OnceLock::new();

pub struct Config {
    setup_dirs: Vec<PathBuf>,
}

pub fn config() -> &'static Config {
    CONFIG.get_or_init(|| Config {
        setup_dirs: Vec::new(),
    })
}
