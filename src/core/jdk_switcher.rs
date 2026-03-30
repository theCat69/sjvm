use std::path::{Path, PathBuf};

use anyhow::{bail, Context};

use crate::infra::config::config;
use crate::infra::memory::memory;
use crate::infra::symlinks::{create_symlink, symlink_path};

/// Result of a JDK lookup operation.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum JdkLookupResult {
    /// JDK was found at the given path.
    Found(PathBuf),
    /// No JDK matching the version string was found.
    NotFound,
}

/// Finds a JDK matching `version` from the cached JDK list.
///
/// Matching is done by checking whether the JDK directory name contains
/// `version` as a substring.
pub(crate) fn find_jdk_by_version(version: &str) -> JdkLookupResult {
    find_jdk_by_version_in_list(version, &memory().jdks)
}

/// Finds a JDK matching `version` in an explicit `jdks` list.
///
/// This is the testable variant that accepts an explicit list instead of
/// reading from the global cache.
pub(crate) fn find_jdk_by_version_in_list(version: &str, jdks: &[PathBuf]) -> JdkLookupResult {
    for jdk in jdks {
        if jdk
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .contains(version)
        {
            return JdkLookupResult::Found(jdk.clone());
        }
    }
    JdkLookupResult::NotFound
}

/// Switches the active JDK by pointing the managed symlink to `jdk_path`.
///
/// Before creating the symlink, the path is canonicalized to prevent TOCTOU
/// races and to verify the target is still inside a configured `jdks_dirs`.
///
/// # Errors
/// Returns an error if the path cannot be canonicalized, if the canonical
/// path is outside all configured `jdks_dirs`, or if symlink creation fails.
pub(crate) fn switch_to_jdk(jdk_path: &Path) -> anyhow::Result<()> {
    // Canonicalize resolves symlinks and `..` components to get the real path.
    let canonical = jdk_path.canonicalize().with_context(|| {
        format!(
            "Failed to canonicalize JDK path '{}'",
            jdk_path.to_string_lossy()
        )
    })?;

    // Verify the canonical path is still inside one of the configured jdks_dirs.
    let cfg = config();
    let in_configured_dir = cfg.jdks_dirs.iter().any(|dir| {
        // Attempt to canonicalize the configured dir; fall back to the raw path
        // if it does not exist yet (e.g. first-run before setup).
        let canonical_dir = PathBuf::from(dir)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(dir));
        canonical.starts_with(&canonical_dir)
    });

    if !in_configured_dir {
        bail!(
            "JDK path '{}' is outside configured jdks_dirs — refusing to create symlink",
            canonical.display()
        );
    }

    let symlink = symlink_path();
    create_symlink(&canonical, &symlink).with_context(|| {
        format!(
            "Failed to switch to JDK at '{}'",
            canonical.to_string_lossy()
        )
    })?;
    Ok(())
}

/// Returns the display name for a JDK path (the final directory component).
pub(crate) fn jdk_display_name(jdk_path: &Path) -> String {
    jdk_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_jdks() -> Vec<PathBuf> {
        vec![
            PathBuf::from("/usr/lib/jvm/temurin-11-jdk"),
            PathBuf::from("/usr/lib/jvm/temurin-17-jdk"),
            PathBuf::from("/usr/lib/jvm/temurin-21-jdk"),
            PathBuf::from("/usr/lib/jvm/graalvm-ce-java17"),
            PathBuf::from("/usr/lib/jvm/zulu-8"),
        ]
    }

    #[test]
    fn test_find_jdk_by_version_exact_match() {
        let jdks = test_jdks();

        let result = find_jdk_by_version_in_list("temurin-17-jdk", &jdks);
        assert_eq!(
            result,
            JdkLookupResult::Found(PathBuf::from("/usr/lib/jvm/temurin-17-jdk"))
        );
    }

    #[test]
    fn test_find_jdk_by_version_partial_match() {
        let jdks = test_jdks();

        // Match by version number
        let result = find_jdk_by_version_in_list("17", &jdks);
        assert_eq!(
            result,
            JdkLookupResult::Found(PathBuf::from("/usr/lib/jvm/temurin-17-jdk"))
        );

        // Match by vendor
        let result = find_jdk_by_version_in_list("graalvm", &jdks);
        assert_eq!(
            result,
            JdkLookupResult::Found(PathBuf::from("/usr/lib/jvm/graalvm-ce-java17"))
        );

        // Match by vendor
        let result = find_jdk_by_version_in_list("zulu", &jdks);
        assert_eq!(
            result,
            JdkLookupResult::Found(PathBuf::from("/usr/lib/jvm/zulu-8"))
        );
    }

    #[test]
    fn test_find_jdk_by_version_not_found() {
        let jdks = test_jdks();

        let result = find_jdk_by_version_in_list("99", &jdks);
        assert_eq!(result, JdkLookupResult::NotFound);

        let result = find_jdk_by_version_in_list("openjdk", &jdks);
        assert_eq!(result, JdkLookupResult::NotFound);
    }

    #[test]
    fn test_find_jdk_by_version_empty_list() {
        let jdks: Vec<PathBuf> = vec![];

        let result = find_jdk_by_version_in_list("17", &jdks);
        assert_eq!(result, JdkLookupResult::NotFound);
    }

    #[test]
    fn test_find_jdk_returns_first_match() {
        let jdks = vec![
            PathBuf::from("/usr/lib/jvm/java-17-openjdk"),
            PathBuf::from("/usr/lib/jvm/temurin-17-jdk"),
        ];

        // Should return the first match
        let result = find_jdk_by_version_in_list("17", &jdks);
        assert_eq!(
            result,
            JdkLookupResult::Found(PathBuf::from("/usr/lib/jvm/java-17-openjdk"))
        );
    }

    #[test]
    fn test_jdk_display_name() {
        assert_eq!(
            jdk_display_name(Path::new("/usr/lib/jvm/temurin-17-jdk")),
            "temurin-17-jdk"
        );
        assert_eq!(
            jdk_display_name(Path::new("/home/user/.sdkman/candidates/java/17.0.1-tem")),
            "17.0.1-tem"
        );
        assert_eq!(jdk_display_name(Path::new("/")), "");
    }

    #[test]
    fn test_jdk_lookup_result_equality() {
        let path = PathBuf::from("/usr/lib/jvm/jdk-17");

        assert_eq!(
            JdkLookupResult::Found(path.clone()),
            JdkLookupResult::Found(path.clone())
        );
        assert_eq!(JdkLookupResult::NotFound, JdkLookupResult::NotFound);
        assert_ne!(JdkLookupResult::Found(path), JdkLookupResult::NotFound);
    }
}
