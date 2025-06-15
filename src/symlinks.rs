use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::config::config;

pub fn get_symlink_path() -> PathBuf {
    PathBuf::from(&config().symlink_dir)
}

pub fn create_symlink(target: &Path, link: &Path) -> Result<(), anyhow::Error> {
    if link.exists() {
        if cfg!(target_os = "windows") {
            std::fs::remove_dir(link).with_context(|| "Cannot remove symlink")?;
        } else {
            println!("Remving symlink linux");
            std::fs::remove_file(link).with_context(|| "Cannot remove symlink")?;
        }
    }

    #[cfg(target_os = "windows")]
    std::os::windows::fs::symlink_dir(target, link).with_context(|| "Cannot create symlink")?;

    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).with_context(|| "Cannot create symlink")?;

    Ok(())
}
