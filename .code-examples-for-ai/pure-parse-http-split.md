<!-- Demonstrates: pure function / HTTP-calling function separation for testability; parse helpers never touch the network -->

```rust
//! JDK artifact catalog — resolves download URLs for OpenJDK (Adoptium) and GraalVM CE.
//!
//! Pure parse functions are separated from HTTP calls so that unit tests never need
//! a live network connection.

use anyhow::{Context, Result, bail};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Pattern: separate pure parse functions from HTTP-calling functions
//
// parse_adoptium_response()  — accepts &Value, fully testable with json!() fixtures
// resolve_adoptium()         — makes HTTP call, then delegates to the pure parser
//
// This split means unit tests exercise all parsing logic without any mocking.
// ---------------------------------------------------------------------------

/// Parses an Adoptium API JSON array response into an ArtifactInfo.
/// Pure function — accepts pre-fetched JSON; never makes HTTP calls.
pub(crate) fn parse_adoptium_response(json: &Value, version: u16) -> Result<ArtifactInfo> {
    let arr = json
        .as_array()
        .context("Adoptium response is not a JSON array")?;

    if arr.is_empty() {
        bail!("Adoptium returned no JDK builds for version {version}");
    }

    let pkg = &arr[0]["binary"]["package"];

    let download_url = pkg["link"]
        .as_str()
        .context("Adoptium response missing required field: binary.package.link")?
        .to_owned();

    let filename = pkg["name"]
        .as_str()
        .context("Adoptium response missing required field: binary.package.name")?
        .to_owned();

    // Optional fields — use Option chaining, no bail!
    let sha256_value = pkg["checksum"].as_str().map(str::to_owned);
    let sha256_url = pkg["checksum_link"].as_str().map(str::to_owned);

    Ok(ArtifactInfo { download_url, sha256_value, sha256_url, filename, vendor: Vendor::OpenJdk, version })
}

/// HTTP-calling wrapper — fetches JSON then delegates to the pure parser.
/// This function is NOT called in unit tests; only in integration / E2E.
fn resolve_adoptium(version: u16, os: &str, arch: &str) -> Result<ArtifactInfo> {
    let url = format!(
        "https://api.adoptium.net/v3/assets/latest/{version}/hotspot\
         ?os={os}&architecture={arch}&image_type=jdk"
    );
    let json = crate::infra::http::get_json(&url)
        .with_context(|| format!("Failed to fetch Adoptium API for JDK {version}"))?;

    // Delegate to the pure function — fully tested independently
    parse_adoptium_response(&json, version)
}

// --- Tests (pure functions only — no network, no mocks needed) --------------
#[cfg(test)]
mod tests {
    use serde_json::json;
    use super::*;

    // Construct fixture JSON inline with the json!() macro
    fn valid_adoptium_json() -> serde_json::Value {
        json!([{
            "binary": {
                "package": {
                    "link": "https://example.com/jdk-21.tar.gz",
                    "checksum": "abcdef1234",
                    "checksum_link": "https://example.com/jdk-21.tar.gz.sha256",
                    "name": "OpenJDK21U-jdk_x64_linux_hotspot_21.0.5_11.tar.gz"
                }
            }
        }])
    }

    #[test]
    fn test_parse_adoptium_response_valid() {
        let json = valid_adoptium_json();
        let artifact = parse_adoptium_response(&json, 21).expect("should parse valid response");
        assert_eq!(artifact.download_url, "https://example.com/jdk-21.tar.gz");
        assert_eq!(artifact.version, 21);
        assert_eq!(artifact.vendor, Vendor::OpenJdk);
    }

    #[test]
    fn test_parse_adoptium_response_empty_array() {
        let json = json!([]);
        let result = parse_adoptium_response(&json, 21);
        assert!(result.is_err());
        // Assert on error message content for human-readable test failures
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("no JDK builds"), "got: {msg}");
    }
}
```
