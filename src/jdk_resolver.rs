use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

static JDKS: OnceLock<Vec<PathBuf>> = OnceLock::new();

pub fn detect_jdks() -> &'static Vec<PathBuf> {
    JDKS.get_or_init(|| {
        let mut jdks = Vec::new();
        let candidates = if cfg!(target_os = "windows") {
            vec![
                "C:\\Program Files\\Java",
                "C:\\dev\\interpreteur_compilateur\\Java",
            ]
        } else if cfg!(target_os = "macos") {
            vec!["/Library/Java/JavaVirtualMachines"]
        } else {
            vec!["/usr/lib/jvm"]
        };

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
