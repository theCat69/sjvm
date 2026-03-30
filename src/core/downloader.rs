//! JDK download, verification and installation pipeline.
//!
//! The main entry-point is [`install_jdk`]. Pure helpers (`verify_checksum`,
//! `extract_tar_gz`, `extract_zip`, `identify_top_level_dir`,
//! `validate_dest_within_jdks_dir`) are exposed `pub(crate)` so that unit
//! tests can exercise them without triggering any HTTP calls.

use std::{
    fs,
    io::Read as _,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

use crate::core::jdk_catalog::ArtifactInfo;
use crate::core::jdk_switcher::vendor_to_str;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Describes a single JDK installation request.
#[allow(dead_code)] // removed in Phase 3 when install command calls this
pub(crate) struct InstallRequest {
    /// Resolved artifact metadata (URL, checksum, filename, …).
    pub(crate) artifact: ArtifactInfo,
    /// Installation root — the first entry in `jdks_dirs` from config.
    pub(crate) dest_dir: PathBuf,
    /// When `true`, overwrite an already-installed JDK of the same name.
    pub(crate) force: bool,
}

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

/// Verifies that the SHA-256 digest of `file_path` matches `expected_hex`.
///
/// `expected_hex` must be exactly 64 lowercase or uppercase hexadecimal characters.
#[allow(dead_code)] // removed in Phase 3 when install command calls this
pub(crate) fn verify_checksum(file_path: &Path, expected_hex: &str) -> Result<()> {
    if expected_hex.len() != 64 || !expected_hex.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("not a valid SHA-256 hex string: '{expected_hex}'");
    }

    let mut file = fs::File::open(file_path)
        .with_context(|| format!("Failed to open file for checksum: {}", file_path.display()))?;

    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 65_536];
    loop {
        let n = file.read(&mut buf).with_context(|| {
            format!("Failed to read file for checksum: {}", file_path.display())
        })?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    let computed = format!("{:x}", hasher.finalize());
    if computed.to_lowercase() != expected_hex.to_lowercase() {
        bail!("SHA-256 checksum mismatch: expected={expected_hex}, computed={computed}");
    }
    Ok(())
}

/// Extracts a `.tar.gz` archive into `dest_dir`, guarding against path traversal.
#[allow(dead_code)] // removed in Phase 3 when install command calls this
pub(crate) fn extract_tar_gz(archive_path: &Path, dest_dir: &Path) -> Result<()> {
    let file = fs::File::open(archive_path)
        .with_context(|| format!("Failed to open archive: {}", archive_path.display()))?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(gz);

    for entry_result in archive
        .entries()
        .context("Failed to read tar archive entries")?
    {
        let mut entry = entry_result.context("Failed to read tar entry")?;
        let entry_path = entry
            .path()
            .context("Tar entry has invalid path")?
            .into_owned();

        let entry_type = entry.header().entry_type();

        // Symlinks and hardlinks require extra traversal checks on the link target
        // before we let the `tar` crate unpack them.
        if entry_type.is_symlink() || entry_type == tar::EntryType::Link {
            let link_target = entry
                .link_name()
                .context("Link entry has invalid link name")?
                .context("Link entry has no link name")?
                .into_owned();

            // For symlinks the target is resolved relative to the symlink's own
            // directory inside dest_dir.  Compute the canonical destination of the
            // resolved target and assert it stays under dest_dir.
            if entry_type.is_symlink() {
                let symlink_parent = dest_dir.join(
                    entry_path
                        .parent()
                        .unwrap_or_else(|| std::path::Path::new("")),
                );
                let resolved = symlink_parent.join(&link_target);
                // Normalise lexically (no canonicalize — target may not exist yet).
                let mut components = Vec::new();
                for component in resolved.components() {
                    match component {
                        std::path::Component::ParentDir => {
                            if components.pop().is_none() {
                                bail!(
                                    "Symlink '{}' → '{}' escapes the extraction directory",
                                    entry_path.display(),
                                    link_target.display()
                                );
                            }
                        }
                        std::path::Component::CurDir => {}
                        c => components.push(c),
                    }
                }
                let normalised: PathBuf = components.iter().collect();
                if !normalised.starts_with(dest_dir) {
                    bail!(
                        "Symlink '{}' → '{}' escapes the extraction directory — aborting to prevent path traversal",
                        entry_path.display(),
                        link_target.display()
                    );
                }
            } else {
                // Hardlinks: the target path is relative to the archive root, i.e.
                // relative to dest_dir.  Apply the same traversal guard used for
                // regular file entries.
                guard_path_traversal(&link_target, dest_dir).with_context(|| {
                    format!(
                        "Hardlink target '{}' failed path traversal check",
                        link_target.display()
                    )
                })?;
            }

            guard_path_traversal(&entry_path, dest_dir)?;
            let dest_entry = dest_dir.join(&entry_path);
            if let Some(parent) = dest_entry.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("Failed to create parent directory: {}", parent.display())
                })?;
            }
            entry.unpack(&dest_entry).with_context(|| {
                format!("Failed to unpack link entry to: {}", dest_entry.display())
            })?;
            continue;
        }

        guard_path_traversal(&entry_path, dest_dir)?;

        let dest_entry = dest_dir.join(&entry_path);
        if entry.header().entry_type().is_dir() {
            fs::create_dir_all(&dest_entry)
                .with_context(|| format!("Failed to create directory: {}", dest_entry.display()))?;
        } else {
            if let Some(parent) = dest_entry.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("Failed to create parent directory: {}", parent.display())
                })?;
            }
            entry
                .unpack(&dest_entry)
                .with_context(|| format!("Failed to unpack entry to: {}", dest_entry.display()))?;
        }
    }
    Ok(())
}

/// Extracts a `.zip` archive into `dest_dir`, guarding against path traversal.
#[allow(dead_code)] // removed in Phase 3 when install command calls this
pub(crate) fn extract_zip(archive_path: &Path, dest_dir: &Path) -> Result<()> {
    let file = fs::File::open(archive_path)
        .with_context(|| format!("Failed to open zip archive: {}", archive_path.display()))?;
    let mut zip = zip::ZipArchive::new(file)
        .with_context(|| format!("Failed to read zip archive: {}", archive_path.display()))?;

    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .with_context(|| format!("Failed to read zip entry at index {i}"))?;

        let entry_path: PathBuf = match entry.enclosed_name() {
            Some(p) => p.to_owned(),
            None => bail!("path traversal detected in archive entry: {}", entry.name()),
        };

        // Check for symlinks — zip unix mode 0xA000 indicates a symlink.
        #[cfg(unix)]
        {
            if let Some(mode) = entry.unix_mode() {
                // Unix file type mask: 0xA000 = symlink
                if mode & 0xF000 == 0xA000 {
                    bail!(
                        "Archive contains a symlink entry '{}' — symlinks are not permitted for security reasons",
                        entry.name()
                    );
                }
            }
        }

        guard_path_traversal(&entry_path, dest_dir)?;

        let dest_entry = dest_dir.join(&entry_path);

        if entry.is_dir() {
            fs::create_dir_all(&dest_entry)
                .with_context(|| format!("Failed to create directory: {}", dest_entry.display()))?;
        } else {
            if let Some(parent) = dest_entry.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("Failed to create parent directory: {}", parent.display())
                })?;
            }
            let mut out = fs::File::create(&dest_entry)
                .with_context(|| format!("Failed to create file: {}", dest_entry.display()))?;
            std::io::copy(&mut entry, &mut out).with_context(|| {
                format!("Failed to write zip entry to: {}", dest_entry.display())
            })?;
        }
    }
    Ok(())
}

/// Returns the single top-level subdirectory inside `dir`.
///
/// Errors if the directory is empty or contains more than one entry.
#[allow(dead_code)] // removed in Phase 3 when install command calls this
pub(crate) fn identify_top_level_dir(dir: &Path) -> Result<PathBuf> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .with_context(|| format!("Failed to read directory: {}", dir.display()))?
        .map(|e| {
            e.with_context(|| format!("Failed to read directory entry in: {}", dir.display()))
                .map(|de| de.path())
        })
        .collect::<Result<Vec<_>>>()?;

    match entries.len() {
        0 => bail!(
            "Extracted archive has no top-level directory in: {}",
            dir.display()
        ),
        1 => {
            let path = entries.remove(0);
            if path.is_dir() {
                Ok(path)
            } else {
                bail!(
                    "Expected top-level directory but found a file: {}",
                    path.display()
                )
            }
        }
        n => bail!(
            "unexpected number of top-level entries ({n}) in extracted archive: {}",
            dir.display()
        ),
    }
}

/// Validates that `dest` is a direct child of `jdks_dir` to prevent path traversal.
///
/// Canonicalizes `jdks_dir` (which must already exist on disk); bails with an error if
/// canonicalization fails rather than silently falling back to a lexical check.  Then
/// verifies that:
///
/// 1. `dest.parent()` resolves to exactly `canonical_jdks` (one level deep, no subdirs).
/// 2. `dest.file_name()` contains no path separators (`/` or `\`).
#[allow(dead_code)] // removed in Phase 3 when install command calls this
pub(crate) fn validate_dest_within_jdks_dir(dest: &Path, jdks_dir: &Path) -> Result<()> {
    // Canonicalize the jdks_dir — bail if it does not exist or is inaccessible.
    let canonical_jdks = jdks_dir.canonicalize().with_context(|| {
        format!(
            "Cannot canonicalize jdks_dir '{}' — directory may not exist",
            jdks_dir.display()
        )
    })?;

    // dest must have a filename component with no path separators.
    let dest_name = dest
        .file_name()
        .and_then(|n| n.to_str())
        .context("Destination path has no filename component")?;

    if dest_name.contains('/') || dest_name.contains('\\') {
        bail!(
            "Destination filename '{}' contains path separators — aborting to prevent path traversal",
            dest_name
        );
    }

    // Reconstruct the expected canonical destination and verify it is a direct child.
    let canonical_dest = canonical_jdks.join(dest_name);

    if canonical_dest.parent() != Some(canonical_jdks.as_path()) {
        bail!(
            "Destination '{}' is outside the configured jdks_dir '{}' — aborting to prevent path traversal",
            dest.display(),
            jdks_dir.display()
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Three-layer path traversal guard for archive entries.
fn guard_path_traversal(entry_path: &Path, dest_dir: &Path) -> Result<()> {
    // Layer 1: reject any ".." component.
    if entry_path
        .components()
        .any(|c| c == std::path::Component::ParentDir)
    {
        bail!(
            "path traversal detected in archive entry: {}",
            entry_path.display()
        );
    }

    // Layer 2: reject absolute paths.
    if entry_path.is_absolute() {
        bail!("absolute path in archive entry: {}", entry_path.display());
    }

    // Layer 3: starts_with check on the joined destination.
    let dest_entry = dest_dir.join(entry_path);
    if !dest_entry.starts_with(dest_dir) {
        bail!(
            "path traversal (starts_with check) in archive entry: {}",
            entry_path.display()
        );
    }

    Ok(())
}

/// Recursively copies the directory tree rooted at `src` into `dst`.
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

        if src_path.is_dir() {
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
// Main orchestration
// ---------------------------------------------------------------------------

/// Downloads, verifies, and installs a JDK described by `request`.
///
/// `on_progress(bytes_downloaded, total)` is called after each download chunk.
/// Returns the path to the installed JDK directory on success.
#[allow(dead_code)] // removed in Phase 3 when install command calls this
pub(crate) fn install_jdk(
    request: InstallRequest,
    on_progress: impl Fn(u64, Option<u64>),
) -> Result<PathBuf> {
    // S2/S3 — sanitize artifact.filename before using it in any path construction.
    // This prevents a malicious server from placing temp files outside /tmp via a
    // filename like "../../etc/cron.d/x.tar.gz".
    let safe_name = Path::new(&request.artifact.filename)
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| !n.contains('/') && !n.contains('\\') && !n.contains('\0'))
        .with_context(|| format!("Invalid artifact filename: {:?}", request.artifact.filename))?;

    // Step 2 — derive the final installation directory name from the archive filename.
    let jdk_dir_name = std::path::Path::new(safe_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.trim_end_matches(".tar"))
        .map(str::to_owned)
        .context("Cannot derive JDK directory name from filename")?;

    let final_dest = request.dest_dir.join(&jdk_dir_name);

    if final_dest.exists() && !request.force {
        bail!(
            "JDK is already installed at '{}'. Use --force to overwrite.",
            final_dest.display()
        );
    }

    // Step 3 — temporary download file (PID-qualified to avoid collisions between
    // concurrent installs of the same version).
    let pid = std::process::id();
    let temp_path = std::env::temp_dir().join(format!("sjvm-download-{}-{}", safe_name, pid));

    // Step 4 — stream-download the archive.
    if let Err(e) = crate::infra::http::download_streaming(
        &request.artifact.download_url,
        &temp_path,
        on_progress,
    ) {
        let _ = fs::remove_file(&temp_path); // best-effort cleanup
        return Err(e).with_context(|| {
            format!(
                "Failed to download JDK from {}",
                request.artifact.download_url
            )
        });
    }

    // Step 5 — checksum verification.
    let checksum_hex: String = if let Some(ref hex) = request.artifact.sha256_value {
        hex.clone()
    } else if let Some(ref url) = request.artifact.sha256_url {
        let raw = crate::infra::http::get_text(url)
            .with_context(|| format!("Failed to fetch checksum from {url}"))?;
        // Take the first whitespace-delimited token (handles "hex  filename" format).
        raw.split_whitespace()
            .next()
            .context("Checksum file was empty")?
            .to_owned()
    } else {
        let _ = fs::remove_file(&temp_path);
        bail!("No checksum available for {}", request.artifact.filename);
    };

    if let Err(e) = verify_checksum(&temp_path, &checksum_hex) {
        let _ = fs::remove_file(&temp_path); // best-effort cleanup on mismatch
        return Err(e);
    }

    // Step 6 — extract archive to a unique temp directory.
    let temp_extract_dir = std::env::temp_dir().join(format!("sjvm-extract-{}-{}", safe_name, pid));

    fs::create_dir_all(&temp_extract_dir).with_context(|| {
        format!(
            "Failed to create temp extract dir: {}",
            temp_extract_dir.display()
        )
    })?;

    let extract_result = if request.artifact.filename.ends_with(".tar.gz") {
        extract_tar_gz(&temp_path, &temp_extract_dir)
    } else if request.artifact.filename.ends_with(".zip") {
        extract_zip(&temp_path, &temp_extract_dir)
    } else {
        bail!("Unsupported archive format: {}", request.artifact.filename);
    };

    if let Err(e) = extract_result {
        let _ = fs::remove_file(&temp_path);
        let _ = fs::remove_dir_all(&temp_extract_dir);
        return Err(e);
    }

    // R2 — wrap all post-extraction steps in a cleanup guard so that temp_extract_dir
    // is removed on any failure after this point.
    let post_extract_result = (|| -> Result<PathBuf> {
        let top_level = identify_top_level_dir(&temp_extract_dir)?;

        // Step 7 — move extracted JDK to final destination.
        validate_dest_within_jdks_dir(&final_dest, &request.dest_dir)?;

        if final_dest.exists() {
            fs::remove_dir_all(&final_dest).with_context(|| {
                format!("Failed to remove existing JDK at {}", final_dest.display())
            })?;
        }

        let rename_result = fs::rename(&top_level, &final_dest);
        match rename_result {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
                copy_dir_all(&top_level, &final_dest)
                    .with_context(|| format!("Failed to copy JDK to {}", final_dest.display()))?;
                let _ = fs::remove_dir_all(&top_level); // best-effort cleanup
            }
            Err(e) => {
                return Err(e)
                    .with_context(|| format!("Failed to move JDK to {}", final_dest.display()));
            }
        }

        // Step 8 — write the `.sjvm-managed` marker file so `list` and the TUI
        // can distinguish sjvm-managed JDKs from manually-installed ones.
        let marker_path = final_dest.join(".sjvm-managed");
        if let Err(e) = std::fs::write(&marker_path, b"") {
            eprintln!("Warning: could not write .sjvm-managed marker: {e}");
        }

        // Step 8b — write the `.sjvm-vendor` marker file containing the lowercase
        // vendor name so that `sjvm use --vendor` can filter by distribution.
        let vendor_name = vendor_to_str(&request.artifact.vendor);
        let vendor_path = final_dest.join(".sjvm-vendor");
        if let Err(e) = std::fs::write(&vendor_path, vendor_name) {
            eprintln!("Warning: could not write .sjvm-vendor marker: {e}");
        }

        // Step 9 — invalidate the in-process JDK discovery cache.
        crate::infra::memory::invalidate_memory();

        Ok(final_dest)
    })();

    if post_extract_result.is_err() {
        let _ = fs::remove_dir_all(&temp_extract_dir);
    }

    // Step 9 — clean up temp files (best-effort, non-fatal).
    let _ = fs::remove_file(&temp_path);
    let _ = fs::remove_dir_all(&temp_extract_dir);

    post_extract_result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::{
        io::Write as _,
        path::{Path, PathBuf},
    };

    use sha2::{Digest, Sha256};

    use super::*;

    // --- Helpers ---

    fn tmp_dir(suffix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sjvm_test_{suffix}_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create test tmp dir");
        dir
    }

    /// Builds a minimal `.tar.gz` archive in memory and writes it to `dest`.
    ///
    /// Each entry is `(name, data)`. If `name` ends with `/` and `data` is empty,
    /// a directory entry is emitted with mode `0o755`; otherwise a regular file
    /// entry is emitted with mode `0o644`.
    fn create_tar_gz(dest: &Path, entries: &[(&str, &[u8])]) {
        let file = std::fs::File::create(dest).expect("create tar.gz file");
        let gz = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut builder = tar::Builder::new(gz);
        for (name, data) in entries {
            if name.ends_with('/') && data.is_empty() {
                // Strip trailing slash for append_dir path argument.
                let dir_name = name.trim_end_matches('/');
                builder
                    .append_dir(dir_name, ".")
                    .expect("append tar dir entry");
            } else {
                let mut header = tar::Header::new_gnu();
                header.set_entry_type(tar::EntryType::Regular);
                header.set_mode(0o644);
                header.set_size(data.len() as u64);
                header.set_cksum();
                builder
                    .append_data(&mut header, name, std::io::Cursor::new(data))
                    .expect("append tar file entry");
            }
        }
        builder.finish().expect("finish tar archive");
    }

    /// Builds a raw ustar tar block for a single file entry with the given name and data.
    ///
    /// This bypasses `tar::Builder`'s path validation so we can create malicious
    /// entries for security tests. The name field is written as raw bytes without
    /// any safety checks.
    fn raw_tar_gz_with_entry(dest: &Path, raw_name: &[u8], data: &[u8]) {
        // ustar header is exactly 512 bytes.
        // Fields (offset, len): name(0,100), mode(100,8), uid(108,8), gid(116,8),
        // size(124,12), mtime(136,12), checksum(148,8), typeflag(156,1),
        // linkname(157,100), magic(257,6), version(263,2), ...
        let mut header = [0u8; 512];

        // name field (bytes 0-99): copy raw name, truncated at 99 chars.
        let name_len = raw_name.len().min(99);
        header[..name_len].copy_from_slice(&raw_name[..name_len]);

        // mode "0000644\0"
        header[100..108].copy_from_slice(b"0000644\0");
        // uid / gid
        header[108..116].copy_from_slice(b"0000000\0");
        header[116..124].copy_from_slice(b"0000000\0");
        // size in octal (11 digits + null)
        let size_oct = format!("{:011o}\0", data.len());
        header[124..136].copy_from_slice(size_oct.as_bytes());
        // mtime
        header[136..148].copy_from_slice(b"00000000000\0");
        // checksum placeholder (8 spaces)
        header[148..156].copy_from_slice(b"        ");
        // typeflag: '0' = regular file
        header[156] = b'0';
        // ustar magic + version
        header[257..263].copy_from_slice(b"ustar ");
        header[263..265].copy_from_slice(b" \0");

        // Compute checksum: sum of all header bytes (spaces in checksum field).
        let checksum: u32 = header.iter().map(|&b| b as u32).sum();
        let cksum_str = format!("{checksum:06o}\0 ");
        header[148..156].copy_from_slice(cksum_str.as_bytes());

        // Pad data to 512-byte block boundary.
        let padded_len = (data.len() + 511) & !511;
        let mut tar_bytes = Vec::with_capacity(512 + padded_len + 1024);
        tar_bytes.extend_from_slice(&header);
        tar_bytes.extend_from_slice(data);
        // Padding to 512-byte block
        let pad = padded_len - data.len();
        tar_bytes.extend(std::iter::repeat(0u8).take(pad));
        // Two 512-byte zero blocks = end-of-archive marker
        tar_bytes.extend(std::iter::repeat(0u8).take(1024));

        // Gzip-compress and write.
        let file = std::fs::File::create(dest).expect("create archive");
        let mut gz = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        gz.write_all(&tar_bytes).expect("write tar bytes");
        gz.finish().expect("finish gzip");
    }

    /// Builds a minimal `.tar.gz` archive containing a single symlink entry.
    ///
    /// `path` is the entry name; `link_target` is the symlink target stored in
    /// the tar link-name field.  This uses a raw ustar header so we can set the
    /// typeflag to `'2'` (symlink) directly.
    fn create_tar_gz_with_symlink(dest: &Path, path: &[u8], link_target: &[u8]) {
        // ustar header — same layout as `raw_tar_gz_with_entry` but typeflag='2'
        // and the link-name field (bytes 157-256) holds the symlink target.
        let mut header = [0u8; 512];

        let name_len = path.len().min(99);
        header[..name_len].copy_from_slice(&path[..name_len]);

        header[100..108].copy_from_slice(b"0000644\0");
        header[108..116].copy_from_slice(b"0000000\0");
        header[116..124].copy_from_slice(b"0000000\0");
        // size = 0 for symlinks
        header[124..136].copy_from_slice(b"00000000000\0");
        header[136..148].copy_from_slice(b"00000000000\0");
        header[148..156].copy_from_slice(b"        "); // checksum placeholder
        // typeflag: '2' = symbolic link
        header[156] = b'2';
        // link-name field (bytes 157-256)
        let link_len = link_target.len().min(99);
        header[157..157 + link_len].copy_from_slice(&link_target[..link_len]);
        // ustar magic + version
        header[257..263].copy_from_slice(b"ustar ");
        header[263..265].copy_from_slice(b" \0");

        let checksum: u32 = header.iter().map(|&b| b as u32).sum();
        let cksum_str = format!("{checksum:06o}\0 ");
        header[148..156].copy_from_slice(cksum_str.as_bytes());

        // End-of-archive marker: two 512-byte zero blocks.
        let mut tar_bytes = Vec::with_capacity(512 + 1024);
        tar_bytes.extend_from_slice(&header);
        tar_bytes.extend(std::iter::repeat(0u8).take(1024));

        let file = std::fs::File::create(dest).expect("create archive");
        let mut gz = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        gz.write_all(&tar_bytes).expect("write tar bytes");
        gz.finish().expect("finish gzip");
    }

    /// Builds a minimal `.zip` archive and writes it to `dest`.
    fn create_zip(dest: &Path, entries: &[(&str, &[u8])]) {
        let file = std::fs::File::create(dest).expect("create zip file");
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in entries {
            zip.start_file(*name, opts).expect("start zip file entry");
            zip.write_all(data).expect("write zip entry data");
        }
        zip.finish().expect("finish zip archive");
    }

    // --- Checksum tests ---

    #[test]
    fn test_checksum_verify_match() {
        let dir = tmp_dir("chk_match");
        let file_path = dir.join("data.bin");
        let data = b"hello sjvm checksum test";
        std::fs::write(&file_path, data).expect("write test file");

        let mut hasher = Sha256::new();
        hasher.update(data);
        let expected = format!("{:x}", hasher.finalize());

        verify_checksum(&file_path, &expected).expect("checksum should match");

        let _ = std::fs::remove_file(&file_path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_checksum_verify_mismatch() {
        let dir = tmp_dir("chk_mismatch");
        let file_path = dir.join("data.bin");
        std::fs::write(&file_path, b"real data").expect("write test file");

        let wrong_hex = "0000000000000000000000000000000000000000000000000000000000000000";
        let result = verify_checksum(&file_path, wrong_hex);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("mismatch"),
            "expected 'mismatch' in error, got: {msg}"
        );

        let _ = std::fs::remove_file(&file_path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_checksum_verify_malformed_hex() {
        let dir = tmp_dir("chk_malformed");
        let file_path = dir.join("data.bin");
        std::fs::write(&file_path, b"data").expect("write test file");

        let result = verify_checksum(&file_path, "xyz");
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("not a valid SHA-256 hex string"),
            "expected 'not a valid SHA-256 hex string' in error, got: {msg}"
        );

        let _ = std::fs::remove_file(&file_path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- tar.gz extraction tests ---

    #[test]
    fn test_extract_tar_gz_rejects_path_traversal() {
        let dir = tmp_dir("tgz_traversal");
        let archive = dir.join("evil.tar.gz");
        let dest = dir.join("dest");
        std::fs::create_dir_all(&dest).expect("create dest dir");

        // Use raw tar builder to bypass tar crate's path validation guards.
        raw_tar_gz_with_entry(&archive, b"../../evil.txt", b"evil");

        let result = extract_tar_gz(&archive, &dest);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("path traversal"),
            "expected 'path traversal' in error, got: {msg}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_extract_tar_gz_rejects_absolute_path() {
        let dir = tmp_dir("tgz_abs");
        let archive = dir.join("abs.tar.gz");
        let dest = dir.join("dest");
        std::fs::create_dir_all(&dest).expect("create dest dir");

        // Use raw tar builder to bypass tar crate's path validation guards.
        raw_tar_gz_with_entry(&archive, b"/etc/passwd", b"secret");

        let result = extract_tar_gz(&archive, &dest);
        assert!(
            result.is_err(),
            "should reject absolute path entry in tar.gz"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_extract_tar_gz_valid() {
        let dir = tmp_dir("tgz_valid");
        let archive = dir.join("valid.tar.gz");
        let dest = dir.join("dest");
        std::fs::create_dir_all(&dest).expect("create dest dir");

        create_tar_gz(
            &archive,
            &[
                ("jdk-21/", &[]),
                ("jdk-21/release", b"JAVA_VERSION=\"21\"\n"),
            ],
        );

        extract_tar_gz(&archive, &dest).expect("should extract valid tar.gz");
        assert!(
            dest.join("jdk-21/release").exists(),
            "release file should exist"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- symlink extraction tests ---

    #[test]
    fn test_extract_tar_gz_rejects_symlink_that_escapes() {
        let dir = tmp_dir("tgz_sym_escape");
        let archive = dir.join("evil_sym.tar.gz");
        let dest = dir.join("dest");
        std::fs::create_dir_all(&dest).expect("create dest dir");

        // Symlink inside "jdk-21/sub/" pointing three levels up — the path
        // "jdk-21/sub/escape" is 3 levels deep inside dest, so "../../../" would
        // resolve to the *parent* of dest, escaping the extraction directory.
        create_tar_gz_with_symlink(&archive, b"jdk-21/sub/escape", b"../../../outside");

        let result = extract_tar_gz(&archive, &dest);
        assert!(
            result.is_err(),
            "should reject symlink that escapes dest_dir"
        );
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("escapes"),
            "expected 'escapes' in error message, got: {msg}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn test_extract_tar_gz_allows_safe_relative_symlink() {
        let dir = tmp_dir("tgz_sym_safe");
        let archive = dir.join("safe_sym.tar.gz");
        let dest = dir.join("dest");
        std::fs::create_dir_all(&dest).expect("create dest dir");

        // Build an archive with a real file and a symlink pointing to it within the tree.
        // jdk-21/a/real.txt  (regular file)
        // jdk-21/b/link.txt -> ../a/real.txt  (symlink that resolves within dest)
        //
        // Use a nested block so that `builder` is dropped (and the underlying
        // GzEncoder is finished/flushed) before we try to read the archive back.
        {
            let file = std::fs::File::create(&archive).expect("create archive");
            let gz = flate2::write::GzEncoder::new(file, flate2::Compression::default());
            let mut builder = tar::Builder::new(gz);

            // Regular file
            let content = b"hello";
            let mut file_header = tar::Header::new_gnu();
            file_header.set_entry_type(tar::EntryType::Regular);
            file_header.set_mode(0o644);
            file_header.set_size(content.len() as u64);
            file_header.set_cksum();
            builder
                .append_data(
                    &mut file_header,
                    "jdk-21/a/real.txt",
                    std::io::Cursor::new(content),
                )
                .expect("append regular file");

            // Symlink entry — do NOT pre-set link_name or cksum; append_link
            // calls prepare_header_path / prepare_header_link / set_cksum
            // internally and would corrupt any pre-set checksum.
            let mut sym_header = tar::Header::new_gnu();
            sym_header.set_entry_type(tar::EntryType::Symlink);
            sym_header.set_size(0);
            sym_header.set_mode(0o777);
            builder
                .append_link(&mut sym_header, "jdk-21/b/link.txt", "../a/real.txt")
                .expect("append symlink");

            // into_inner writes the EOF blocks and returns the GzEncoder.
            // We then call finish() on the encoder so the gzip trailer is
            // flushed to disk before the block ends.
            let gz = builder.into_inner().expect("finalise tar builder");
            gz.finish().expect("finish gzip encoder");
        } // file is closed here

        extract_tar_gz(&archive, &dest).expect("safe symlink should be allowed");

        // The symlink should exist and point to the right target.
        let link_path = dest.join("jdk-21/b/link.txt");
        let target = std::fs::read_link(&link_path).expect("should be a symlink");
        assert_eq!(target, std::path::Path::new("../a/real.txt"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- zip extraction tests ---

    #[test]
    fn test_extract_zip_rejects_path_traversal() {
        let dir = tmp_dir("zip_traversal");
        let archive = dir.join("evil.zip");
        let dest = dir.join("dest");
        std::fs::create_dir_all(&dest).expect("create dest dir");

        create_zip(&archive, &[("../evil.txt", b"evil")]);

        let result = extract_zip(&archive, &dest);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("path traversal") || msg.contains("enclosed_name"),
            "expected traversal error, got: {msg}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_extract_zip_valid() {
        let dir = tmp_dir("zip_valid");
        let archive = dir.join("valid.zip");
        let dest = dir.join("dest");
        std::fs::create_dir_all(&dest).expect("create dest dir");

        create_zip(
            &archive,
            &[
                ("jdk-21/", &[]),
                ("jdk-21/release", b"JAVA_VERSION=\"21\"\n"),
            ],
        );

        extract_zip(&archive, &dest).expect("should extract valid zip");
        assert!(
            dest.join("jdk-21/release").exists(),
            "release file should exist"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- identify_top_level_dir tests ---

    #[test]
    fn test_identify_top_level_dir_single() {
        let dir = tmp_dir("top_single");
        let subdir = dir.join("jdk-21");
        std::fs::create_dir_all(&subdir).expect("create subdir");

        let result = identify_top_level_dir(&dir).expect("should find single subdir");
        assert_eq!(result, subdir);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_identify_top_level_dir_multiple() {
        let dir = tmp_dir("top_multi");
        std::fs::create_dir_all(dir.join("jdk-21")).expect("create subdir 1");
        std::fs::create_dir_all(dir.join("jdk-17")).expect("create subdir 2");

        let result = identify_top_level_dir(&dir);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("unexpected"),
            "expected 'unexpected' in error, got: {msg}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_identify_top_level_dir_empty() {
        let dir = tmp_dir("top_empty");

        let result = identify_top_level_dir(&dir);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("no top-level directory") || msg.contains("no top-level"),
            "expected 'no top-level' in error, got: {msg}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- validate_dest_within_jdks_dir tests ---

    #[test]
    fn test_destination_path_within_jdks_dir() {
        let jdks_dir = tmp_dir("validate_within");
        let dest = jdks_dir.join("jdk-21");
        validate_dest_within_jdks_dir(&dest, &jdks_dir)
            .expect("dest inside jdks_dir should be valid");

        let _ = std::fs::remove_dir_all(&jdks_dir);
    }

    #[test]
    fn test_destination_path_escapes_jdks_dir() {
        let jdks_dir = tmp_dir("validate_escape");

        // A dest with no filename component (e.g. bare root) must be rejected
        // with a "no filename" error.
        let dest_no_filename = PathBuf::from("/");
        let result = validate_dest_within_jdks_dir(&dest_no_filename, &jdks_dir);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("no filename"),
            "expected 'no filename' in error, got: {msg}"
        );

        let _ = std::fs::remove_dir_all(&jdks_dir);
    }

    #[test]
    fn test_destination_path_nonexistent_jdks_dir_fails() {
        // If jdks_dir does not exist, canonicalize fails and we must get an error
        // rather than a silent fallback.
        let jdks_dir = PathBuf::from("/nonexistent/path/that/does/not/exist/sjvm/jdks");
        let dest = jdks_dir.join("jdk-21");
        let result = validate_dest_within_jdks_dir(&dest, &jdks_dir);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("Cannot canonicalize"),
            "expected 'Cannot canonicalize' in error, got: {msg}"
        );
    }
}
