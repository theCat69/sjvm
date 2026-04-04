use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Context as _;

use crate::infra::config::config;

/// Returns the path of the managed JDK symlink as configured in `symlink_dir`.
///
/// This is the path that should be added to `PATH` and set as `JAVA_HOME`.
pub(crate) fn symlink_path() -> PathBuf {
    PathBuf::from(&config().symlink_dir)
}

/// Removes an existing symlink (or directory junction on Windows) at `link`,
/// tolerating the case where it is already absent.
///
/// # Errors
/// Returns an error if the path exists but cannot be removed.
fn remove_existing_link(link: &Path) -> anyhow::Result<()> {
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
    Ok(())
}

/// Creates (or replaces) a directory symlink at `link` pointing to `target`.
///
/// Ensures the parent directory of `link` exists before creating the symlink.
/// Removal of any existing symlink is performed unconditionally before
/// creation to avoid a TOCTOU race between an existence check and the
/// remove call.
///
/// # Errors
/// Returns an error if the existing symlink cannot be removed (for reasons
/// other than it being absent), if the parent directory cannot be created,
/// or if symlink creation fails.
pub(crate) fn create_symlink(target: &Path, link: &Path) -> anyhow::Result<()> {
    // Ensure the parent directory of the symlink exists.
    if let Some(parent) = link.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create parent directory for symlink: {}",
                parent.display()
            )
        })?;
    }

    remove_existing_link(link)?;

    #[cfg(target_os = "windows")]
    std::os::windows::fs::symlink_dir(target, link)
        .map_err(|e| anyhow::anyhow!(e).context("Cannot create symlink"))?;

    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link)
        .map_err(|e| anyhow::anyhow!(e).context("Cannot create symlink"))?;

    Ok(())
}
