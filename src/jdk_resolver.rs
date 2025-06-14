use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use crate::config::config;

static JDKS: OnceLock<Vec<PathBuf>> = OnceLock::new();

pub fn detect_jdks() -> &'static Vec<PathBuf> {
    JDKS.get_or_init(|| {
        let config = config();
        let mut jdks = Vec::new();
        let candidates = &config.setup_dirs;

        for base in candidates {
            let path = Path::new(base);
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        jdks.push(p);
                    }
                }
            }
        }

        jdks
    })
}
