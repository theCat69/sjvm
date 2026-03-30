use anyhow::Context;

use crate::infra::memory::memory;
use crate::infra::symlinks::symlink_path;

/// Lists all known JDKs, marking the currently active one with `→`.
///
/// JDKs that were not installed by sjvm (no `.sjvm-managed` marker) are
/// annotated with `[custom]`.
///
/// # Errors
/// Returns an error if the current symlink cannot be read.
pub(crate) fn list_versions() -> anyhow::Result<()> {
    let current_link = symlink_path();
    let current = std::fs::read_link(&current_link)
        .with_context(|| format!("Cannot read current symlink '{}'", current_link.display()))?;

    for jdk in memory().jdks {
        let is_current = jdk == current;
        let marker = if is_current { "→" } else { " " };
        let custom_tag = if jdk.join(".sjvm-managed").exists() {
            ""
        } else {
            " [custom]"
        };
        println!("{} {}{}", marker, jdk.display(), custom_tag);
    }

    Ok(())
}
