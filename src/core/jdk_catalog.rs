//! JDK artifact catalog — resolves download URLs for OpenJDK (Adoptium) and GraalVM CE.
//!
//! Pure parse functions are separated from HTTP calls so that unit tests never need
//! a live network connection.

use anyhow::{Context, Result, bail};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// JDK distribution vendor.
#[derive(Debug, Clone, PartialEq, clap::ValueEnum)]
pub(crate) enum Vendor {
    /// Eclipse Temurin OpenJDK distribution via Adoptium API.
    #[value(name = "openjdk")]
    OpenJdk,
    /// GraalVM Community Edition via GitHub Releases API.
    #[value(name = "graalvm")]
    GraalVm,
}

/// Resolved download metadata for a single JDK artifact.
#[derive(Debug, Clone)]
#[allow(dead_code)] // removed in Phase 3 when install command calls this
pub(crate) struct ArtifactInfo {
    /// Direct HTTPS download URL for the archive.
    pub(crate) download_url: String,
    /// Inline SHA-256 hex checksum from the API response (preferred).
    pub(crate) sha256_value: Option<String>,
    /// Fallback URL to fetch the SHA-256 checksum from.
    pub(crate) sha256_url: Option<String>,
    /// Archive filename (e.g. `OpenJDK21U-jdk_x64_linux_hotspot_21.0.5_11.tar.gz`).
    pub(crate) filename: String,
    /// Which vendor produced this artifact.
    pub(crate) vendor: Vendor,
    /// JDK major version (e.g. `21`).
    pub(crate) version: u16,
}

// ---------------------------------------------------------------------------
// OS / arch detection
// ---------------------------------------------------------------------------

/// Maps [`std::env::consts::OS`] to the Adoptium API `os` parameter.
///
/// Adoptium uses `mac` (not `macos`) for macOS.
#[allow(dead_code)] // removed in Phase 3 when install command calls this
pub(crate) fn detect_os() -> Result<String> {
    let os_str = std::env::consts::OS;
    let mapped = match os_str {
        "linux" => "linux",
        "macos" => "mac",
        "windows" => "windows",
        other => bail!("Unsupported OS: {other}. Supported: linux, macos, windows"),
    };
    Ok(mapped.to_owned())
}

/// Maps [`std::env::consts::ARCH`] to the Adoptium/GraalVM API `architecture` parameter.
#[allow(dead_code)] // removed in Phase 3 when install command calls this
pub(crate) fn detect_arch() -> Result<String> {
    let arch_str = std::env::consts::ARCH;
    let mapped = match arch_str {
        "x86_64" => "x64",
        "aarch64" => "aarch64",
        other => bail!("Unsupported architecture: {other}. Supported: x86_64/aarch64"),
    };
    Ok(mapped.to_owned())
}

// ---------------------------------------------------------------------------
// Adoptium response parser
// ---------------------------------------------------------------------------

/// Parses an Adoptium API JSON array response into an [`ArtifactInfo`].
///
/// Expects the full array response (`json` is `&Value::Array`). Index `[0]` is used.
#[allow(dead_code)] // removed in Phase 3 when install command calls this
pub(crate) fn parse_adoptium_response(json: &Value, version: u16) -> Result<ArtifactInfo> {
    let arr = json
        .as_array()
        .context("Adoptium response is not a JSON array")?;

    if arr.is_empty() {
        bail!("Adoptium returned no JDK builds for version {version}");
    }

    let entry = &arr[0];
    let pkg = &entry["binary"]["package"];

    let download_url = pkg["link"]
        .as_str()
        .context("Adoptium response missing required field: binary.package.link")?
        .to_owned();

    let filename = pkg["name"]
        .as_str()
        .context("Adoptium response missing required field: binary.package.name")?
        .to_owned();

    let sha256_value = pkg["checksum"].as_str().map(str::to_owned);
    let sha256_url = pkg["checksum_link"].as_str().map(str::to_owned);

    Ok(ArtifactInfo {
        download_url,
        sha256_value,
        sha256_url,
        filename,
        vendor: Vendor::OpenJdk,
        version,
    })
}

// ---------------------------------------------------------------------------
// GraalVM CE response parser
// ---------------------------------------------------------------------------

/// Returns the expected asset filename for a GraalVM CE release.
///
/// `version_str` is the full version string extracted from the tag (e.g. `"21.0.5"`).
fn graalvm_asset_name(version_str: &str, os: &str, arch: &str) -> String {
    // GraalVM uses "macos" (not "mac") in asset names.
    // Extension: tar.gz for linux/macos, zip for windows.
    let ext = if os == "windows" { "zip" } else { "tar.gz" };
    format!("graalvm-community-jdk-{version_str}_{os}-{arch}_bin.{ext}")
}

/// Parses a GraalVM CE GitHub Releases API JSON array into an [`ArtifactInfo`].
///
/// `json` is the full releases array (already fetched, one page).  
/// `version` is the desired JDK major version.  
/// `os` / `arch` are the GraalVM-flavoured tokens (`linux`/`macos`/`windows`, `x64`/`aarch64`).
#[allow(dead_code)] // removed in Phase 3 when install command calls this
pub(crate) fn parse_graalvm_releases(
    json: &Value,
    version: u16,
    os: &str,
    arch: &str,
) -> Result<ArtifactInfo> {
    let releases = json
        .as_array()
        .context("GraalVM releases response is not a JSON array")?;

    // Find the release whose tag starts with "jdk-{version}."
    let tag_prefix = format!("jdk-{version}.");
    let release = releases
        .iter()
        .find(|r| {
            r["tag_name"]
                .as_str()
                .map(|t| t.starts_with(&tag_prefix))
                .unwrap_or(false)
        })
        .ok_or_else(|| anyhow::anyhow!("No GraalVM CE release found for JDK {version}"))?;

    // Extract full version string from the tag, e.g. "jdk-21.0.5" → "21.0.5"
    let tag = release["tag_name"]
        .as_str()
        .context("GraalVM release tag_name is not a string")?;
    let version_str = tag
        .strip_prefix("jdk-")
        .context("Unexpected GraalVM tag format — expected 'jdk-X.Y.Z'")?;

    let expected_name = graalvm_asset_name(version_str, os, arch);

    let assets = release["assets"]
        .as_array()
        .context("GraalVM release 'assets' is not a JSON array")?;

    // Find the primary binary asset.
    let asset = assets
        .iter()
        .find(|a| {
            a["name"]
                .as_str()
                .map(|n| n == expected_name)
                .unwrap_or(false)
        })
        .ok_or_else(|| {
            anyhow::anyhow!("No GraalVM CE asset found for version={version} os={os} arch={arch}")
        })?;

    let download_url = asset["browser_download_url"]
        .as_str()
        .context("GraalVM asset missing 'browser_download_url'")?
        .to_owned();

    let filename = asset["name"]
        .as_str()
        .context("GraalVM asset missing 'name'")?
        .to_owned();

    // Inline checksum: "digest" field has format "sha256:{hex}" — strip prefix.
    let sha256_value = asset["digest"]
        .as_str()
        .and_then(|d| d.strip_prefix("sha256:"))
        .map(str::to_owned);

    // Sidecar checksum: look for an asset whose name ends with ".sha256" matching our base name.
    let sha256_base = format!("{expected_name}.sha256");
    let sha256_url = assets
        .iter()
        .find(|a| {
            a["name"]
                .as_str()
                .map(|n| n == sha256_base)
                .unwrap_or(false)
        })
        .and_then(|a| a["browser_download_url"].as_str())
        .map(str::to_owned);

    Ok(ArtifactInfo {
        download_url,
        sha256_value,
        sha256_url,
        filename,
        vendor: Vendor::GraalVm,
        version,
    })
}

// ---------------------------------------------------------------------------
// HTTP resolver
// ---------------------------------------------------------------------------

/// Resolves download metadata by querying the vendor's API over HTTPS.
///
/// This function performs real HTTP calls and should not be called in unit tests.
#[allow(dead_code)] // removed in Phase 3 when install command calls this
pub(crate) fn resolve_artifact(
    vendor: &Vendor,
    version: u16,
    os: &str,
    arch: &str,
) -> Result<ArtifactInfo> {
    match vendor {
        Vendor::OpenJdk => resolve_adoptium(version, os, arch),
        Vendor::GraalVm => resolve_graalvm(version, os, arch),
    }
}

fn resolve_adoptium(version: u16, os: &str, arch: &str) -> Result<ArtifactInfo> {
    let url = format!(
        "https://api.adoptium.net/v3/assets/latest/{version}/hotspot\
         ?os={os}&architecture={arch}&image_type=jdk&jvm_impl=hotspot&vendor=eclipse"
    );
    debug_assert!(url.starts_with("https://"), "Adoptium URL must be HTTPS");

    let json = crate::infra::http::get_json(&url)
        .with_context(|| format!("Failed to fetch Adoptium API for JDK {version}"))?;

    parse_adoptium_response(&json, version)
}

fn resolve_graalvm(version: u16, os: &str, arch: &str) -> Result<ArtifactInfo> {
    // GraalVM uses "macos" in asset names; the Adoptium os token "mac" must not be passed here.
    // The caller (resolve_artifact) passes the raw `os` from detect_os() which returns "mac"
    // for macOS. We must re-map to "macos" for GraalVM asset name matching.
    let graalvm_os = match os {
        "mac" => "macos",
        other => other,
    };

    const BASE_URL: &str =
        "https://api.github.com/repos/graalvm/graalvm-ce-builds/releases?per_page=100";
    const MAX_PAGES: u32 = 5;

    for page in 1..=MAX_PAGES {
        let url = if page == 1 {
            BASE_URL.to_owned()
        } else {
            format!("{BASE_URL}&page={page}")
        };
        debug_assert!(url.starts_with("https://"), "GraalVM URL must be HTTPS");

        let json = crate::infra::http::get_json(&url)
            .with_context(|| format!("Failed to fetch GraalVM releases page {page}"))?;

        let releases = json
            .as_array()
            .context("GraalVM releases response is not a JSON array")?;

        if releases.is_empty() {
            break;
        }

        let tag_prefix = format!("jdk-{version}.");
        let found = releases.iter().any(|r| {
            r["tag_name"]
                .as_str()
                .map(|t| t.starts_with(&tag_prefix))
                .unwrap_or(false)
        });

        if found {
            return parse_graalvm_releases(&json, version, graalvm_os, arch)
                .with_context(|| format!("Failed to parse GraalVM release for JDK {version}"));
        }
    }

    bail!("No GraalVM CE release found for JDK {version}");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    // --- OS detection ---

    #[cfg(target_os = "linux")]
    #[test]
    fn test_detect_os_linux() {
        let result = detect_os().expect("detect_os should succeed on linux");
        assert_eq!(result, "linux");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_detect_os_macos() {
        let result = detect_os().expect("detect_os should succeed on macos");
        assert_eq!(result, "mac");
    }

    // --- Arch detection ---

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_detect_arch_x86_64() {
        let result = detect_arch().expect("detect_arch should succeed on x86_64");
        assert_eq!(result, "x64");
    }

    #[cfg(target_arch = "aarch64")]
    #[test]
    fn test_detect_arch_aarch64() {
        let result = detect_arch().expect("detect_arch should succeed on aarch64");
        assert_eq!(result, "aarch64");
    }

    // --- Adoptium response parsing ---

    fn adoptium_valid_json() -> serde_json::Value {
        json!([{
            "binary": {
                "package": {
                    "link": "https://example.com/jdk-21.tar.gz",
                    "checksum": "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
                    "checksum_link": "https://example.com/jdk-21.tar.gz.sha256",
                    "name": "OpenJDK21U-jdk_x64_linux_hotspot_21.0.5_11.tar.gz"
                }
            },
            "version": { "major": 21 }
        }])
    }

    #[test]
    fn test_parse_adoptium_response_valid() {
        let json = adoptium_valid_json();
        let artifact = parse_adoptium_response(&json, 21).expect("should parse valid response");
        assert_eq!(artifact.download_url, "https://example.com/jdk-21.tar.gz");
        assert_eq!(
            artifact.sha256_value,
            Some("abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890".to_owned())
        );
        assert_eq!(
            artifact.sha256_url,
            Some("https://example.com/jdk-21.tar.gz.sha256".to_owned())
        );
        assert_eq!(
            artifact.filename,
            "OpenJDK21U-jdk_x64_linux_hotspot_21.0.5_11.tar.gz"
        );
        assert_eq!(artifact.version, 21);
        assert_eq!(artifact.vendor, Vendor::OpenJdk);
    }

    #[test]
    fn test_parse_adoptium_response_empty_array() {
        let json = json!([]);
        let result = parse_adoptium_response(&json, 21);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("no JDK builds"),
            "expected 'no JDK builds' in error, got: {msg}"
        );
    }

    #[test]
    fn test_parse_adoptium_response_missing_checksum() {
        // checksum absent, checksum_link present → sha256_value is None, sha256_url is Some
        let json = json!([{
            "binary": {
                "package": {
                    "link": "https://example.com/jdk-21.tar.gz",
                    "checksum_link": "https://example.com/jdk-21.tar.gz.sha256",
                    "name": "jdk-21.tar.gz"
                }
            }
        }]);
        let artifact = parse_adoptium_response(&json, 21).expect("should parse without checksum");
        assert!(artifact.sha256_value.is_none());
        assert_eq!(
            artifact.sha256_url,
            Some("https://example.com/jdk-21.tar.gz.sha256".to_owned())
        );
    }

    #[test]
    fn test_parse_adoptium_response_malformed() {
        // Missing binary.package.link
        let json = json!([{
            "binary": {
                "package": {
                    "name": "jdk-21.tar.gz"
                }
            }
        }]);
        let result = parse_adoptium_response(&json, 21);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("binary.package.link"),
            "expected field path in error, got: {msg}"
        );
    }

    // --- GraalVM releases parsing ---

    fn graalvm_releases_json(tag: &str, asset_name: &str) -> serde_json::Value {
        json!([{
            "tag_name": tag,
            "assets": [
                {
                    "name": asset_name,
                    "browser_download_url": format!("https://github.com/graalvm/releases/download/{tag}/{asset_name}"),
                    "digest": "sha256:aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899"
                },
                {
                    "name": format!("{asset_name}.sha256"),
                    "browser_download_url": format!("https://github.com/graalvm/releases/download/{tag}/{asset_name}.sha256")
                }
            ]
        }])
    }

    #[test]
    fn test_parse_graalvm_releases_finds_correct_tag() {
        let asset = "graalvm-community-jdk-21.0.5_linux-x64_bin.tar.gz";
        let json = graalvm_releases_json("jdk-21.0.5", asset);
        let artifact = parse_graalvm_releases(&json, 21, "linux", "x64")
            .expect("should find matching release");
        assert_eq!(artifact.version, 21);
        assert_eq!(artifact.filename, asset);
    }

    #[test]
    fn test_parse_graalvm_releases_no_matching_tag() {
        let asset = "graalvm-community-jdk-17.0.9_linux-x64_bin.tar.gz";
        let json = graalvm_releases_json("jdk-17.0.9", asset);
        // Looking for version 21 but only jdk-17 exists
        let result = parse_graalvm_releases(&json, 21, "linux", "x64");
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("No GraalVM CE release found"),
            "expected 'No GraalVM CE release found' in error, got: {msg}"
        );
    }

    #[test]
    fn test_parse_graalvm_asset_linux_x64() {
        let asset = "graalvm-community-jdk-21.0.5_linux-x64_bin.tar.gz";
        let json = graalvm_releases_json("jdk-21.0.5", asset);
        let artifact = parse_graalvm_releases(&json, 21, "linux", "x64")
            .expect("should parse linux x64 asset");
        assert_eq!(
            artifact.download_url,
            "https://github.com/graalvm/releases/download/jdk-21.0.5/graalvm-community-jdk-21.0.5_linux-x64_bin.tar.gz"
        );
        assert_eq!(
            artifact.sha256_value,
            Some("aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899".to_owned())
        );
        assert_eq!(
            artifact.sha256_url,
            Some(format!(
                "https://github.com/graalvm/releases/download/jdk-21.0.5/{asset}.sha256"
            ))
        );
        assert_eq!(artifact.vendor, Vendor::GraalVm);
    }

    #[test]
    fn test_parse_graalvm_asset_not_found_for_arch() {
        // Only linux-x64 asset present; ask for aarch64 → not found
        let asset = "graalvm-community-jdk-21.0.5_linux-x64_bin.tar.gz";
        let json = graalvm_releases_json("jdk-21.0.5", asset);
        let result = parse_graalvm_releases(&json, 21, "linux", "aarch64");
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("No GraalVM CE asset found"),
            "expected 'No GraalVM CE asset found' in error, got: {msg}"
        );
    }

    // --- URL validation (delegate to http.rs require_https, tested here conceptually) ---

    #[test]
    fn test_url_validation_rejects_http() {
        // Validates that get_json rejects non-HTTPS URLs before any network call.
        let result = crate::infra::http::get_json("http://api.adoptium.net/test");
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("non-HTTPS"),
            "expected 'non-HTTPS' in error, got: {msg}"
        );
    }

    #[test]
    fn test_url_validation_accepts_https() {
        // An HTTPS URL passes the scheme check — it will fail at the network level,
        // not at the validation level. We confirm the error is NOT about "non-HTTPS".
        let result = crate::infra::http::get_json("https://127.0.0.1:1/nonexistent");
        // Should fail, but NOT because of non-HTTPS check
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            !err_msg.contains("non-HTTPS"),
            "HTTPS URL should not be rejected by scheme check, got: {err_msg}"
        );
    }
}
