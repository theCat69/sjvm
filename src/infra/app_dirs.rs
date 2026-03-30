use anyhow::{Context, bail};
use directories::ProjectDirs;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

static DIRS: OnceLock<AppDirs> = OnceLock::new();

/// Platform-specific directories used by sjvm for data and configuration storage.
pub(crate) struct AppDirs {
    /// Directory for persistent data files (e.g. the binary JDK cache `sjvm-mem`).
    /// Follows the XDG Base Directory spec on Linux (`~/.local/share/sjvm`).
    pub(crate) data_dir: PathBuf,
    /// Directory for configuration files (e.g. `sjvm-conf.json`).
    /// Follows the XDG Base Directory spec on Linux (`~/.config/sjvm`).
    pub(crate) config_dir: PathBuf,
}

/// Returns the application directories, initialising them on first call.
///
/// # Panics
/// Panics at program startup if platform directories cannot be created; this
/// is intentional — the binary cannot function without them.
pub(crate) fn app_dirs() -> &'static AppDirs {
    DIRS.get_or_init(|| init_app_dirs().expect("Failed to initialise application directories"))
}

fn init_app_dirs() -> anyhow::Result<AppDirs> {
    let proj_dirs = init_proj_dir()?;
    Ok(AppDirs {
        data_dir: ensure_dir(proj_dirs.data_dir())?,
        config_dir: ensure_dir(proj_dirs.config_dir())?,
    })
}

fn init_proj_dir() -> anyhow::Result<ProjectDirs> {
    ProjectDirs::from("", "sjvm", "sjvm").context("Failed to resolve platform project directories")
}

fn ensure_dir(path: &std::path::Path) -> anyhow::Result<PathBuf> {
    if path.as_os_str().is_empty() {
        bail!("Application directory path is empty");
    }
    fs::create_dir_all(path)
        .with_context(|| format!("Failed to create directory '{}'", path.display()))?;

    // Set owner-only permissions (0700) to prevent other users from reading
    // sjvm's config and cache data.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .with_context(|| format!("Failed to set permissions on '{}'", path.display()))?;
    }

    Ok(path.to_path_buf())
}
