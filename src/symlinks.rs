use std::path::{Path, PathBuf};

use crate::config::config;

/// Returns the path of the managed JDK symlink as configured in `symlink_dir`.
///
/// This is the path that should be added to `PATH` and set as `JAVA_HOME`.
pub(crate) fn symlink_path() -> PathBuf {
    PathBuf::from(&config().symlink_dir)
}

/// Creates (or replaces) a directory symlink at `link` pointing to `target`.
///
/// Removal of any existing symlink is performed unconditionally before
/// creation to avoid a TOCTOU race between an existence check and the
/// remove call.
///
/// # Errors
/// Returns an error if the existing symlink cannot be removed (for reasons
/// other than it being absent) or if symlink creation fails.
pub(crate) fn create_symlink(target: &Path, link: &Path) -> anyhow::Result<()> {
    // Unconditional removal avoids a TOCTOU race between exists() and remove.
    #[cfg(target_os = "windows")]
    {
        match std::fs::remove_dir(link) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(anyhow::anyhow!(e)
                    .context("failed to remove existing path at symlink location"));
            }
        }
    }
    #[cfg(unix)]
    {
        match std::fs::remove_file(link) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(anyhow::anyhow!(e)
                    .context("failed to remove existing path at symlink location"));
            }
        }
    }

    #[cfg(target_os = "windows")]
    std::os::windows::fs::symlink_dir(target, link)
        .map_err(|e| anyhow::anyhow!(e).context("Cannot create symlink"))?;

    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link)
        .map_err(|e| anyhow::anyhow!(e).context("Cannot create symlink"))?;

    Ok(())
}
