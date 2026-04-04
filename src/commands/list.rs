use anyhow::Context;

use crate::infra::memory::memory;
use crate::infra::symlinks::symlink_path;

/// Lists all known JDKs, marking the currently active one with `→`.
///
/// JDKs that were not installed by sjvm (no `.sjvm-managed` marker) are
/// annotated with `[custom]`.
///
/// A missing or unreadable symlink is treated as "no current JDK" rather than
/// an error; the full list is still displayed without any `→` marker.
pub(crate) fn list_versions() -> anyhow::Result<()> {
    let current_link = symlink_path();
    // None if the symlink is absent or cannot be read (e.g. first-run before `sjvm use`).
    let current = std::fs::read_link(&current_link).ok();

    for jdk in memory().context("Failed to load JDK cache")?.jdks {
        let is_current = current.as_ref().map(|c| c == &jdk).unwrap_or(false);
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    // Tests for the marker selection logic used in list_versions.
    // These are pure logic tests — no calls to list_versions (which requires runtime).

    #[test]
    fn test_list_marker_is_arrow_when_current() {
        let jdk = PathBuf::from("/usr/lib/jvm/temurin-17");
        let current = Some(jdk.clone());
        let is_current = current.as_ref().map(|c| c == &jdk).unwrap_or(false);
        assert!(is_current);
        let marker = if is_current { "→" } else { " " };
        assert_eq!(marker, "→");
    }

    #[test]
    fn test_list_marker_is_space_when_no_current() {
        let jdk = PathBuf::from("/usr/lib/jvm/temurin-17");
        let current: Option<PathBuf> = None;
        let is_current = current.as_ref().map(|c| c == &jdk).unwrap_or(false);
        assert!(!is_current);
        let marker = if is_current { "→" } else { " " };
        assert_eq!(marker, " ");
    }

    #[test]
    fn test_list_marker_is_space_when_different_current() {
        let jdk = PathBuf::from("/usr/lib/jvm/temurin-17");
        let current = Some(PathBuf::from("/usr/lib/jvm/temurin-21"));
        let is_current = current.as_ref().map(|c| c == &jdk).unwrap_or(false);
        assert!(!is_current);
        let marker = if is_current { "→" } else { " " };
        assert_eq!(marker, " ");
    }
}
