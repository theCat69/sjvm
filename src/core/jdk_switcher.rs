use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, bail};

use crate::core::jdk_catalog::Vendor;
use crate::infra::config::config;
use crate::infra::memory::memory;
use crate::infra::symlinks::{create_symlink, symlink_path};

/// Returns the vendor name recorded in the `.sjvm-vendor` file inside `jdk_dir`,
/// or `None` if the file does not exist or cannot be read.
fn read_vendor_file(jdk_dir: &Path) -> Option<String> {
    let vendor_path = jdk_dir.join(".sjvm-vendor");
    fs::read_to_string(&vendor_path)
        .ok()
        .map(|s| s.trim().to_owned())
}

/// Converts a [`Vendor`] to its canonical lowercase string token.
pub(crate) fn vendor_to_str(vendor: &Vendor) -> &'static str {
    match vendor {
        Vendor::OpenJdk => "openjdk",
        Vendor::GraalVm => "graalvm",
    }
}

/// Finds all JDKs matching `version` from the cached JDK list, optionally
/// filtered by `vendor_filter`.
///
/// Matching is done by checking whether the JDK directory name contains
/// `version` as a substring. Vendor filtering rules:
/// - JDK has `.sjvm-vendor` matching the filter → included
/// - JDK has `.sjvm-vendor` NOT matching the filter → excluded
/// - JDK has NO `.sjvm-vendor` (custom JDK) → always included
pub(crate) fn find_jdk_by_version(version: &str, vendor_filter: Option<&Vendor>) -> Vec<PathBuf> {
    find_jdk_by_version_in_list(version, &memory().jdks, vendor_filter)
}

/// Finds all JDKs matching `version` in an explicit `jdks` list, optionally
/// filtered by `vendor_filter`.
///
/// This is the testable variant that accepts an explicit list instead of
/// reading from the global cache.
pub(crate) fn find_jdk_by_version_in_list(
    version: &str,
    jdks: &[PathBuf],
    vendor_filter: Option<&Vendor>,
) -> Vec<PathBuf> {
    let mut matches = Vec::new();
    for jdk in jdks {
        let name = jdk.file_name().unwrap_or_default().to_string_lossy();
        if !name.contains(version) {
            continue;
        }
        // Apply vendor filter when one is specified.
        if let Some(filter) = vendor_filter {
            let filter_str = vendor_to_str(filter);
            match read_vendor_file(jdk) {
                Some(ref recorded) if recorded == filter_str => {
                    // Vendor matches — include.
                    matches.push(jdk.clone());
                }
                Some(_) => {
                    // Vendor does not match — exclude.
                }
                None => {
                    // No vendor file (custom JDK) — always include.
                    matches.push(jdk.clone());
                }
            }
        } else {
            matches.push(jdk.clone());
        }
    }
    matches
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
    use std::path::PathBuf;

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
        let result = find_jdk_by_version_in_list("temurin-17-jdk", &jdks, None);
        assert_eq!(result, vec![PathBuf::from("/usr/lib/jvm/temurin-17-jdk")]);
    }

    #[test]
    fn test_find_jdk_by_version_partial_match() {
        let jdks = test_jdks();

        // Match by version number — returns all matching (temurin-17 and graalvm-ce-java17).
        let result = find_jdk_by_version_in_list("17", &jdks, None);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&PathBuf::from("/usr/lib/jvm/temurin-17-jdk")));
        assert!(result.contains(&PathBuf::from("/usr/lib/jvm/graalvm-ce-java17")));

        // Match by vendor substring.
        let result = find_jdk_by_version_in_list("graalvm", &jdks, None);
        assert_eq!(
            result,
            vec![PathBuf::from("/usr/lib/jvm/graalvm-ce-java17")]
        );

        // Match by vendor substring.
        let result = find_jdk_by_version_in_list("zulu", &jdks, None);
        assert_eq!(result, vec![PathBuf::from("/usr/lib/jvm/zulu-8")]);
    }

    #[test]
    fn test_find_jdk_by_version_not_found() {
        let jdks = test_jdks();
        let result = find_jdk_by_version_in_list("99", &jdks, None);
        assert!(result.is_empty());

        let result = find_jdk_by_version_in_list("openjdk", &jdks, None);
        assert!(result.is_empty());
    }

    #[test]
    fn test_find_jdk_by_version_empty_list() {
        let jdks: Vec<PathBuf> = vec![];
        let result = find_jdk_by_version_in_list("17", &jdks, None);
        assert!(result.is_empty());
    }

    #[test]
    fn test_find_jdk_returns_all_matches() {
        let jdks = vec![
            PathBuf::from("/usr/lib/jvm/java-17-openjdk"),
            PathBuf::from("/usr/lib/jvm/temurin-17-jdk"),
        ];

        // Should return ALL matches (both contain "17").
        let result = find_jdk_by_version_in_list("17", &jdks, None);
        assert_eq!(result.len(), 2);
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
    fn test_vendor_to_str() {
        assert_eq!(vendor_to_str(&Vendor::OpenJdk), "openjdk");
        assert_eq!(vendor_to_str(&Vendor::GraalVm), "graalvm");
    }

    fn tmp_test_dir(suffix: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "sjvm_switcher_test_{suffix}_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create test tmp dir");
        dir
    }

    #[test]
    fn test_find_jdk_vendor_filter_excludes_managed_different_vendor() {
        let dir = tmp_test_dir("vendor_exclude");
        let jdk_openjdk = dir.join("jdk-17-openjdk");
        let jdk_graalvm = dir.join("jdk-17-graalvm");
        std::fs::create_dir_all(&jdk_openjdk).unwrap();
        std::fs::create_dir_all(&jdk_graalvm).unwrap();
        std::fs::write(jdk_openjdk.join(".sjvm-vendor"), "openjdk").unwrap();
        std::fs::write(jdk_graalvm.join(".sjvm-vendor"), "graalvm").unwrap();

        let jdks = vec![jdk_openjdk.clone(), jdk_graalvm.clone()];

        // Filter by openjdk — should exclude graalvm-tagged entry.
        let result = find_jdk_by_version_in_list("17", &jdks, Some(&Vendor::OpenJdk));
        assert_eq!(result, vec![jdk_openjdk.clone()]);

        // Filter by graalvm — should exclude openjdk-tagged entry.
        let result = find_jdk_by_version_in_list("17", &jdks, Some(&Vendor::GraalVm));
        assert_eq!(result, vec![jdk_graalvm.clone()]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_find_jdk_vendor_filter_includes_custom_no_vendor_file() {
        let dir = tmp_test_dir("vendor_custom");
        let jdk_custom = dir.join("jdk-17-custom");
        std::fs::create_dir_all(&jdk_custom).unwrap();
        // No .sjvm-vendor file — this is a "custom" JDK.

        let jdks = vec![jdk_custom.clone()];

        // Even with a vendor filter, custom JDK (no .sjvm-vendor) must be included.
        let result = find_jdk_by_version_in_list("17", &jdks, Some(&Vendor::OpenJdk));
        assert_eq!(result, vec![jdk_custom.clone()]);

        let result = find_jdk_by_version_in_list("17", &jdks, Some(&Vendor::GraalVm));
        assert_eq!(result, vec![jdk_custom.clone()]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_find_jdk_vendor_filter_returns_all_matching() {
        let dir = tmp_test_dir("vendor_all");
        let jdk_a = dir.join("jdk-21-a");
        let jdk_b = dir.join("jdk-21-b");
        let jdk_c = dir.join("jdk-21-c");
        std::fs::create_dir_all(&jdk_a).unwrap();
        std::fs::create_dir_all(&jdk_b).unwrap();
        std::fs::create_dir_all(&jdk_c).unwrap();
        std::fs::write(jdk_a.join(".sjvm-vendor"), "openjdk").unwrap();
        std::fs::write(jdk_b.join(".sjvm-vendor"), "openjdk").unwrap();
        // jdk_c has no vendor file (custom).

        let jdks = vec![jdk_a.clone(), jdk_b.clone(), jdk_c.clone()];

        // No filter — all three returned.
        let result = find_jdk_by_version_in_list("21", &jdks, None);
        assert_eq!(result.len(), 3);

        // Filter openjdk — tagged openjdk (a, b) + custom (c) = 3 entries.
        let result = find_jdk_by_version_in_list("21", &jdks, Some(&Vendor::OpenJdk));
        assert_eq!(result.len(), 3);
        assert!(result.contains(&jdk_a));
        assert!(result.contains(&jdk_b));
        assert!(result.contains(&jdk_c));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
