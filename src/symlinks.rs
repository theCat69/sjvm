use std::path::{Path, PathBuf};

use crate::config::config;

pub fn get_symlink_path() -> PathBuf {
    PathBuf::from(&config().symlink_dir)
}

pub fn create_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    if link.exists() {
        #[cfg(target_os = "windows")]
        std::fs::remove_dir(link)?;
        #[cfg(unix)]
        std::fs::remove_file(link)?;
    }

    #[cfg(target_os = "windows")]
    std::os::windows::fs::symlink_dir(target, link)?;

    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link)?;

    Ok(())
}
