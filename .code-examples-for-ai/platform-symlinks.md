<!-- Demonstrates: platform-specific #[cfg(target_os)] symlink guards; atomic replace pattern; cross-device rename fallback -->

```rust
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Cross-platform symlink creation with atomic replace
//
// - Use compile-time #[cfg(target_os)] / #[cfg(unix)] guards for dead-code elimination
// - remove_existing_link handles the pre-existing link before creating the new one
// - Atomic replace: remove → create; if the creation fails, the old link is already gone
//   (acceptable trade-off; a full atomic approach requires OS-specific calls)
// ---------------------------------------------------------------------------

/// Returns the path where the managed symlink should live.
/// Reads from the global Config — never hardcode paths.
pub(crate) fn symlink_path() -> PathBuf {
    PathBuf::from(&crate::infra::config::config().symlink_dir)
}

/// Removes the existing symlink at `link` if it exists.
/// Non-destructive: succeeds silently if the link is already absent.
pub(crate) fn remove_existing_link(link: &Path) -> Result<()> {
    if link.exists() || link.symlink_metadata().is_ok() {
        #[cfg(unix)]
        std::fs::remove_file(link)
            .with_context(|| format!("Failed to remove existing symlink at {}", link.display()))?;

        #[cfg(target_os = "windows")]
        {
            // On Windows a symlink to a directory is a junction — use remove_dir
            if link.is_dir() {
                std::fs::remove_dir(link).with_context(|| {
                    format!("Failed to remove existing junction at {}", link.display())
                })?;
            } else {
                std::fs::remove_file(link).with_context(|| {
                    format!("Failed to remove existing symlink at {}", link.display())
                })?;
            }
        }
    }
    Ok(())
}

/// Creates a symlink at `link` pointing to `target`.
/// Any existing link at `link` is removed first (atomic-ish replace).
pub(crate) fn create_symlink(target: &Path, link: &Path) -> Result<()> {
    remove_existing_link(link)?;

    // Ensure parent directory exists
    if let Some(parent) = link.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create symlink parent directory: {}", parent.display()))?;
    }

    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link)
        .with_context(|| format!("Failed to create symlink {} → {}", link.display(), target.display()))?;

    #[cfg(target_os = "windows")]
    std::os::windows::fs::symlink_dir(target, link)
        .with_context(|| format!("Failed to create directory symlink {} → {}", link.display(), target.display()))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Cross-device rename fallback (used in downloader.rs after extract)
//
// fs::rename fails across device boundaries (EXDEV). Fall back to copy+delete.
// ---------------------------------------------------------------------------

fn move_or_copy(src: &Path, dst: &Path) -> Result<()> {
    match std::fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(e) if e.raw_os_error() == Some(libc::EXDEV) => {
            // Cross-device move: copy the directory tree then remove the source
            copy_dir_all(src, dst)?;
            std::fs::remove_dir_all(src)
                .with_context(|| format!("Failed to remove source after cross-device copy: {}", src.display()))?;
            Ok(())
        }
        Err(e) => Err(e).with_context(|| format!("Failed to rename {} → {}", src.display(), dst.display())),
    }
}
```
