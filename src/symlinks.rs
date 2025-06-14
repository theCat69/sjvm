use directories::UserDirs;
use std::path::{Path, PathBuf};

pub fn get_symlink_path() -> PathBuf {
    if cfg!(target_os = "windows") {
        PathBuf::from("C:\\Java\\current")
    } else {
        if let Some(user_dirs) = UserDirs::new() {
            user_dirs.home_dir().join(".java").join("current")
        } else {
            panic!("OMG no home directories ? wtf dude")
        }
    }
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
