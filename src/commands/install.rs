//! `sjvm install` command — downloads and installs a JDK from Adoptium or GraalVM CE.

use std::{
    fs,
    io::{self, IsTerminal},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use indicatif::{ProgressBar, ProgressStyle};

use crate::core::downloader::{
    InstallRequest, extract_tar_gz, identify_top_level_dir, install_jdk,
    validate_dest_within_jdks_dir,
};
use crate::core::jdk_catalog::{Vendor, detect_arch, detect_os, resolve_artifact};
use crate::core::jdk_switcher::vendor_to_str;
use crate::infra::config::config;
use crate::infra::memory::invalidate_memory;

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
/// 1. If `local_archive` is `Some(path)`, installs from that local `.tar.gz` directly,
///    bypassing the vendor API and network entirely.
/// 2. Otherwise: resolves `os` and `arch` (auto-detected unless overridden), queries
///    the vendor API for download metadata, then downloads, verifies, extracts, and
///    moves the JDK into the first configured `jdks_dirs`.
/// 3. Optionally switches to the newly installed JDK when running in a terminal.
pub(crate) fn run_install(
    version: &str,
    vendor: &Vendor,
    os_override: Option<&str>,
    arch_override: Option<&str>,
    force: bool,
    local_archive: Option<PathBuf>,
) -> Result<()> {
    // Use the first configured JDK directory as the installation root.
    let install_dir = PathBuf::from(
        config()
            .jdks_dirs
            .first()
            .context("No JDKs directory configured — run 'sjvm setup' first")?,
    );

    if let Some(archive_path) = local_archive {
        return install_from_local_archive(version, vendor, &archive_path, &install_dir, force);
    }

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

    // Build install request.
    let request = InstallRequest {
        artifact,
        dest_dir: install_dir,
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
        .unwrap_or_else(|e| {
            eprintln!("⚠️  Warning: progress bar template error: {e}");
            ProgressStyle::default_bar()
        })
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
        io::stdout().flush().context("Failed to flush stdout")?;

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

/// Installs a JDK from a local `.tar.gz` archive, bypassing vendor API and network.
///
/// 1. Validates that `archive_path` exists and is a file.
/// 2. Extracts the tarball into a temporary directory.
/// 3. Identifies the top-level directory name within the archive.
/// 4. Moves (or copies) the extracted JDK into `install_dir/<top_level_name>/`.
/// 5. Writes `.sjvm-vendor` and `.sjvm-managed` marker files.
/// 6. Invalidates the in-process JDK discovery cache.
fn install_from_local_archive(
    _version: &str,
    vendor: &Vendor,
    archive_path: &Path,
    install_dir: &Path,
    force: bool,
) -> Result<()> {
    if !archive_path.exists() {
        bail!("Local archive does not exist: {}", archive_path.display());
    }
    if !archive_path.is_file() {
        bail!(
            "Local archive path is not a file: {}",
            archive_path.display()
        );
    }

    println!(
        "📦 Installing from local archive: {}",
        archive_path.display()
    );

    // Extract to a PID-qualified temp directory to avoid collisions.
    let pid = std::process::id();
    let archive_name = archive_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("local-archive");
    let temp_extract_dir =
        std::env::temp_dir().join(format!("sjvm-local-extract-{}-{}", archive_name, pid));

    fs::create_dir_all(&temp_extract_dir).with_context(|| {
        format!(
            "Failed to create temp extract dir: {}",
            temp_extract_dir.display()
        )
    })?;

    // Extract the tarball.
    let extract_result = extract_tar_gz(archive_path, &temp_extract_dir);
    if let Err(e) = extract_result {
        let _ = fs::remove_dir_all(&temp_extract_dir);
        return Err(e).with_context(|| {
            format!(
                "Failed to extract local archive: {}",
                archive_path.display()
            )
        });
    }

    // Wrap all post-extraction steps in a cleanup guard.
    let post_extract_result = (|| -> Result<PathBuf> {
        // Identify the single top-level directory inside the archive.
        let top_level = identify_top_level_dir(&temp_extract_dir)?;

        let installed_name = top_level
            .file_name()
            .and_then(|n| n.to_str())
            .with_context(|| {
                format!(
                    "Cannot determine JDK directory name from archive top-level: {}",
                    top_level.display()
                )
            })?
            .to_owned();

        let final_dest = install_dir.join(&installed_name);

        // Validate the destination is a direct child of install_dir (path traversal guard).
        validate_dest_within_jdks_dir(&final_dest, install_dir)?;

        if final_dest.exists() && !force {
            bail!(
                "JDK is already installed at '{}'. Use --force to overwrite.",
                final_dest.display()
            );
        }

        if final_dest.exists() {
            fs::remove_dir_all(&final_dest).with_context(|| {
                format!("Failed to remove existing JDK at {}", final_dest.display())
            })?;
        }

        // Move extracted JDK to final destination (with cross-device fallback).
        let rename_result = fs::rename(&top_level, &final_dest);
        match rename_result {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
                copy_dir_all(&top_level, &final_dest)
                    .with_context(|| format!("Failed to copy JDK to {}", final_dest.display()))?;
                let _ = fs::remove_dir_all(&top_level);
            }
            Err(e) => {
                return Err(e)
                    .with_context(|| format!("Failed to move JDK to {}", final_dest.display()));
            }
        }

        // Write `.sjvm-managed` marker.
        let marker_path = final_dest.join(".sjvm-managed");
        if let Err(e) = fs::write(&marker_path, b"") {
            eprintln!("Warning: could not write .sjvm-managed marker: {e}");
        }

        // Write `.sjvm-vendor` marker.
        let vendor_name = vendor_to_str(vendor);
        let vendor_path = final_dest.join(".sjvm-vendor");
        if let Err(e) = fs::write(&vendor_path, vendor_name) {
            eprintln!("Warning: could not write .sjvm-vendor marker: {e}");
        }

        // Invalidate the in-process JDK discovery cache.
        invalidate_memory();

        Ok(final_dest)
    })();

    let _ = fs::remove_dir_all(&temp_extract_dir);

    let installed_path = post_extract_result?;
    println!("✅ Installed JDK: {}", installed_path.display());

    Ok(())
}

/// Recursively copies the directory tree rooted at `src` into `dst`,
/// preserving symlinks instead of following them.
fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)
        .with_context(|| format!("Failed to create destination directory: {}", dst.display()))?;

    for entry_result in fs::read_dir(src)
        .with_context(|| format!("Failed to read source directory: {}", src.display()))?
    {
        let entry = entry_result
            .with_context(|| format!("Failed to read directory entry in: {}", src.display()))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        // Use symlink_metadata so we detect symlinks rather than following them.
        let metadata = fs::symlink_metadata(&src_path)
            .with_context(|| format!("Failed to read metadata for: {}", src_path.display()))?;
        let file_type = metadata.file_type();

        if file_type.is_symlink() {
            let target = fs::read_link(&src_path).with_context(|| {
                format!("Failed to read symlink target of: {}", src_path.display())
            })?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(&target, &dst_path).with_context(|| {
                format!(
                    "Failed to create symlink '{}' → '{}'",
                    dst_path.display(),
                    target.display()
                )
            })?;
            #[cfg(windows)]
            {
                // On Windows, distinguish file vs dir symlinks.
                if target.is_dir() {
                    std::os::windows::fs::symlink_dir(&target, &dst_path).with_context(|| {
                        format!(
                            "Failed to create dir symlink '{}' → '{}'",
                            dst_path.display(),
                            target.display()
                        )
                    })?;
                } else {
                    std::os::windows::fs::symlink_file(&target, &dst_path).with_context(|| {
                        format!(
                            "Failed to create file symlink '{}' → '{}'",
                            dst_path.display(),
                            target.display()
                        )
                    })?;
                }
            }
        } else if file_type.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path).with_context(|| {
                format!(
                    "Failed to copy '{}' → '{}'",
                    src_path.display(),
                    dst_path.display()
                )
            })?;
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
