<!-- Demonstrates: anyhow error handling — with_context, bail!, ensure!, ? propagation, no secrets in context -->

```rust
use anyhow::{bail, ensure, Context, Result};
use std::path::Path;

// Standard import pattern for anyhow: all four items in one use statement
// use anyhow::{bail, ensure, Context, Result};

/// Resolves a JDK artifact by calling the vendor API.
///
/// # Errors
/// Returns an error if the HTTP request fails or the response is malformed.
pub(crate) fn resolve_artifact(vendor: &Vendor, version: u16, os: &str) -> Result<ArtifactInfo> {
    // .with_context() — lazy closure, preferred when formatting a dynamic message
    let json = crate::infra::http::get_json(&url)
        .with_context(|| format!("Failed to fetch Adoptium API for JDK {version}"))?;

    // .context() — static string, no allocation when Ok
    let arr = json.as_array().context("Adoptium response is not a JSON array")?;

    if arr.is_empty() {
        // bail! — early-return an Err with a formatted message (like return Err(anyhow!(...)))
        bail!("Adoptium returned no JDK builds for version {version}");
    }

    // ensure! — like assert!, but returns Err instead of panicking
    ensure!(version >= 8 && version <= 25, "version {version} is out of supported range 8–25");

    let download_url = arr[0]["binary"]["package"]["link"]
        .as_str()
        // Option -> Result: .context() on None
        .context("Adoptium response missing required field: binary.package.link")?
        .to_owned();

    Ok(ArtifactInfo { download_url, version, vendor: vendor.clone() })
}

/// Validates that a config path contains no path traversal or NUL bytes.
///
/// # Errors
/// Returns an error describing the invalid component found.
fn validate_no_traversal(p: &str, field: &str) -> Result<()> {
    // Do NOT attach secret values (tokens, paths with credentials) as context strings —
    // they appear in error logs and terminal output.
    if p.contains('\0') {
        bail!("config field '{field}' contains a NUL byte which is not allowed");
    }
    let path = std::path::PathBuf::from(p);
    if path.components().any(|c| c == std::path::Component::ParentDir) {
        bail!("config field '{field}' contains path traversal ('..') which is not allowed");
    }
    Ok(())
}

// --- Tests ------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    // In tests, use anyhow::Result<()> as return type to use ? without unwrap()
    #[test]
    fn test_validate_no_traversal_rejects_dotdot() -> anyhow::Result<()> {
        assert!(validate_no_traversal("/usr/../etc/passwd", "field").is_err());
        assert!(validate_no_traversal("../secret", "field").is_err());
        Ok(())
    }

    // For error-message assertions, use .to_string().contains(...)
    #[test]
    fn test_error_message_contains_field_name() {
        let err = validate_no_traversal("../bad", "symlink_dir").unwrap_err();
        assert!(
            err.to_string().contains("symlink_dir"),
            "expected field name in error, got: {err}"
        );
    }
}
```
