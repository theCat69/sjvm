use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use crate::config::config;

static JDKS: OnceLock<Vec<PathBuf>> = OnceLock::new();

/// Returns all JDK directories found in the configured search paths.
///
/// Results are cached after the first call.
pub(crate) fn detect_jdks() -> &'static Vec<PathBuf> {
    JDKS.get_or_init(|| {
        let config = config();
        let mut jdks = Vec::new();

        for base in &config.jdks_dirs {
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
