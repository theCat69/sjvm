//! `sjvm install` command — downloads and installs a JDK from Adoptium or GraalVM CE.

use std::{
    io::{self, IsTerminal},
    path::PathBuf,
};

use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};

use crate::core::downloader::{InstallRequest, install_jdk};
use crate::core::jdk_catalog::{Vendor, detect_arch, detect_os, resolve_artifact};
use crate::infra::config::config;

/// Validates and normalises a JDK version string for the `install` command.
///
/// If the leading token is an integer it must be in the range 8–25.
/// Non-numeric strings (e.g. `"temurin-21"`) pass through unchanged.
pub(crate) fn validate_install_version(s: &str) -> Result<String, String> {
    crate::commands::validate_version_string(s)?;

    // If the string starts with digits, apply the supported-range check.
    let leading_digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    if !leading_digits.is_empty()
        && let Ok(major) = leading_digits.parse::<u16>()
        && !(8..=25).contains(&major)
    {
        return Err(format!(
            "JDK version {major} is out of range — supported versions: 8–25"
        ));
    }

    Ok(s.to_owned())
}

/// Thin CLI handler for `sjvm install`.
///
/// 1. Resolves `os` and `arch` (auto-detected unless overridden).
/// 2. Parses the leading numeric token as the JDK major version.
/// 3. Queries the vendor API for download metadata.
/// 4. Downloads, verifies, extracts, and moves the JDK into the first
///    configured `jdks_dirs`.
/// 5. Optionally switches to the newly installed JDK when running in a terminal.
pub(crate) fn run_install(
    version: &str,
    vendor: &Vendor,
    os_override: Option<&str>,
    arch_override: Option<&str>,
    force: bool,
) -> Result<()> {
    // Resolve OS and arch.
    let os = match os_override {
        Some(s) => s.to_owned(),
        None => detect_os().context("Failed to detect operating system")?,
    };
    let arch = match arch_override {
        Some(s) => s.to_owned(),
        None => detect_arch().context("Failed to detect CPU architecture")?,
    };

    // Parse the leading digits as the JDK major version number.
    let version_num: u16 = version
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .with_context(|| format!("Cannot parse JDK major version from '{version}'"))?;

    // Resolve artifact metadata from the vendor API.
    println!("🔍 Resolving JDK {version_num} from {vendor:?}...");
    let artifact = resolve_artifact(vendor, version_num, &os, &arch)
        .with_context(|| format!("Failed to fetch JDK catalog for version {version_num}"))?;

    // Use the first configured JDK directory as the installation root.
    let dest_dir = PathBuf::from(
        config()
            .jdks_dirs
            .first()
            .context("No JDKs directory configured — run 'sjvm setup' first")?,
    );

    // Build install request.
    let request = InstallRequest {
        artifact,
        dest_dir,
        force,
    };
    // Use artifact.version (the resolved JDK major version from the vendor API)
    // as the authoritative version for error messages and progress output.
    let artifact_version = request.artifact.version;

    // Set up the progress bar.
    let pb = ProgressBar::new(0);
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] \
             {bytes}/{total_bytes} ({eta})",
        )
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("#>-"),
    );

    let filename_for_display = request.artifact.filename.clone();

    println!("⬇️  Downloading {}...", filename_for_display);

    // Download, verify, extract, and move the JDK into place.
    let installed_path = install_jdk(request, |downloaded, total| {
        if let Some(t) = total
            && (pb.length() == Some(0) || pb.length().is_none())
        {
            pb.set_length(t);
        }
        pb.set_position(downloaded);
    })
    .with_context(|| format!("Failed to install JDK {artifact_version}"))?;

    pb.finish_and_clear();

    println!(
        "✅ Installed {} → {}",
        filename_for_display,
        installed_path.display()
    );

    // Post-install: offer to switch when running interactively.
    if io::stdin().is_terminal() {
        print!("Switch to the newly installed JDK now? [y/N] ");
        use std::io::Write as _;
        io::stdout().flush().ok();

        let mut answer = String::new();
        if io::stdin().read_line(&mut answer).is_ok() {
            let answer = answer.trim();
            if answer.eq_ignore_ascii_case("y") {
                crate::core::jdk_switcher::switch_to_jdk(&installed_path)
                    .with_context(|| format!("Failed to switch to {}", installed_path.display()))?;
                println!("✅ Now using JDK: {}", installed_path.display());
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::validate_install_version;

    #[test]
    fn test_validate_install_version_accepts_8_to_25() {
        for v in [8u16, 11, 17, 21, 25] {
            let s = v.to_string();
            assert!(
                validate_install_version(&s).is_ok(),
                "expected ok for version '{s}'"
            );
        }
    }

    #[test]
    fn test_validate_install_version_rejects_below_8() {
        for v in ["7", "1", "0"] {
            let result = validate_install_version(v);
            assert!(
                result.is_err(),
                "expected error for version '{v}' (below 8)"
            );
            let msg = result.unwrap_err();
            assert!(
                msg.contains("out of range"),
                "expected 'out of range' in error for '{v}', got: {msg}"
            );
        }
    }

    #[test]
    fn test_validate_install_version_rejects_above_25() {
        for v in ["26", "99", "100"] {
            let result = validate_install_version(v);
            assert!(
                result.is_err(),
                "expected error for version '{v}' (above 25)"
            );
            let msg = result.unwrap_err();
            assert!(
                msg.contains("out of range"),
                "expected 'out of range' in error for '{v}', got: {msg}"
            );
        }
    }

    #[test]
    fn test_validate_install_version_non_numeric() {
        // Non-numeric prefixed strings bypass the range check entirely.
        let result = validate_install_version("temurin-17");
        assert!(
            result.is_ok(),
            "expected ok for 'temurin-17', got: {:?}",
            result
        );
        assert_eq!(result.unwrap(), "temurin-17");
    }
}
