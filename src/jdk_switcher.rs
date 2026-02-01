use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::memory::memory;
use crate::symlinks::{create_symlink, get_symlink_path};

/// Result of a JDK lookup operation
#[derive(Debug, Clone, PartialEq)]
pub enum JdkLookupResult {
    /// JDK was found at the given path
    Found(PathBuf),
    /// No JDK matching the version string was found
    NotFound,
}

/// Result of a JDK switch operation
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum SwitchResult {
    /// Successfully switched to the JDK at the given path
    Switched(PathBuf),
    /// No JDK matching the version string was found
    NotFound,
}

/// Finds a JDK matching the given version string from the known JDKs.
///
/// The matching is done by checking if the JDK directory name contains the version string.
///
/// # Arguments
/// * `version` - A version string to search for (e.g., "17", "21", "temurin-11")
///
/// # Returns
/// * `JdkLookupResult::Found(path)` if a matching JDK was found
/// * `JdkLookupResult::NotFound` if no matching JDK was found
pub fn find_jdk_by_version(version: &str) -> JdkLookupResult {
    find_jdk_by_version_in_list(version, &memory().jdks)
}

/// Finds a JDK matching the given version string from a provided list.
///
/// This is the testable version that accepts an explicit list of JDKs.
///
/// # Arguments
/// * `version` - A version string to search for
/// * `jdks` - List of JDK paths to search in
///
/// # Returns
/// * `JdkLookupResult::Found(path)` if a matching JDK was found
/// * `JdkLookupResult::NotFound` if no matching JDK was found
pub fn find_jdk_by_version_in_list(version: &str, jdks: &[PathBuf]) -> JdkLookupResult {
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

/// Switches to the JDK at the given path by creating a symlink.
///
/// # Arguments
/// * `jdk_path` - The path to the JDK to switch to
///
/// # Returns
/// * `Ok(())` if the switch was successful
/// * `Err` if the symlink creation failed
pub fn switch_to_jdk(jdk_path: &Path) -> Result<(), anyhow::Error> {
    let symlink = get_symlink_path();
    create_symlink(jdk_path, &symlink).with_context(|| {
        format!(
            "Failed to switch to JDK at '{}'",
            jdk_path.to_string_lossy()
        )
    })?;
    Ok(())
}

/// Finds and switches to a JDK matching the given version string.
///
/// This combines the lookup and switch operations into a single function.
///
/// # Arguments
/// * `version` - A version string to search for (e.g., "17", "21", "temurin-11")
///
/// # Returns
/// * `Ok(SwitchResult::Switched(path))` if a matching JDK was found and switched to
/// * `Ok(SwitchResult::NotFound)` if no matching JDK was found
/// * `Err` if the symlink creation failed
#[allow(dead_code)]
pub fn switch_to_version(version: &str) -> Result<SwitchResult, anyhow::Error> {
    match find_jdk_by_version(version) {
        JdkLookupResult::Found(jdk_path) => {
            switch_to_jdk(&jdk_path)?;
            Ok(SwitchResult::Switched(jdk_path))
        }
        JdkLookupResult::NotFound => Ok(SwitchResult::NotFound),
    }
}

/// Gets the display name for a JDK path.
///
/// # Arguments
/// * `jdk_path` - The path to the JDK
///
/// # Returns
/// The file name of the JDK directory as a string
pub fn get_jdk_display_name(jdk_path: &Path) -> String {
    jdk_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
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
    fn test_get_jdk_display_name() {
        assert_eq!(
            get_jdk_display_name(Path::new("/usr/lib/jvm/temurin-17-jdk")),
            "temurin-17-jdk"
        );
        assert_eq!(
            get_jdk_display_name(Path::new("/home/user/.sdkman/candidates/java/17.0.1-tem")),
            "17.0.1-tem"
        );
        assert_eq!(get_jdk_display_name(Path::new("/")), "");
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

    #[test]
    fn test_switch_result_equality() {
        let path = PathBuf::from("/usr/lib/jvm/jdk-17");

        assert_eq!(
            SwitchResult::Switched(path.clone()),
            SwitchResult::Switched(path.clone())
        );
        assert_eq!(SwitchResult::NotFound, SwitchResult::NotFound);
        assert_ne!(SwitchResult::Switched(path), SwitchResult::NotFound);
    }
}
