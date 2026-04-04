---
name: project-security
description: Project-specific security guidelines for secrets, input validation, dependencies, auth, and common vulnerabilities
---

# Project Security Guidelines — sjvm

Security best practices for **sjvm** — a Rust CLI tool for managing Java JDK symlinks.

---

## Secrets Management

- **Never hardcode secrets** (API keys, tokens, passwords) in source code or configuration files.
- Use environment variables for any credentials: `std::env::var("GITHUB_TOKEN")`.
- Never commit `.env` files — they are in `.gitignore`.
- **Critical — anyhow context strings**: do not attach secret values (tokens, passwords, private credential paths) as `.context("...")` strings. They appear in terminal output and error logs.
  ```rust
  // WRONG — token visible in error output
  fetch(url).with_context(|| format!("request failed, token={}", token))?;

  // CORRECT — context without secrets
  fetch(url).context("request to GitHub releases API failed")?;
  ```
- **Sensitive HTTP headers**: use `HeaderValue::set_sensitive(true)` for any header carrying credentials. This prevents the value from appearing in `Debug` output:
  ```rust
  let mut auth_value = HeaderValue::from_str(&bearer)
      .context("GITHUB_TOKEN contains characters invalid for an HTTP header value")?;
  auth_value.set_sensitive(true);  // Redacted in logs/debug output
  default_headers.insert(AUTHORIZATION, auth_value);
  ```
- `GITHUB_TOKEN` must never appear in error messages, tracing spans, or log output.

---

## Input Validation

### Path arguments — path traversal prevention

All user-supplied or config-supplied paths must pass `validate_no_traversal()` before use:

```rust
fn validate_no_traversal(p: &str, field: &str) -> anyhow::Result<()> {
    if p.contains('\0') {
        bail!("config field '{field}' contains a NUL byte which is not allowed");
    }
    let path = PathBuf::from(p);
    if path.components().any(|c| c == Component::ParentDir) {
        bail!("config field '{field}' contains path traversal ('..') which is not allowed");
    }
    Ok(())
}
```

After path traversal check, use `canonicalize()` + `starts_with()` to enforce directory bounds:

```rust
// Verify the destination is within the expected jdks_dirs — prevents escape
let canonical_dest = dest.canonicalize()
    .with_context(|| format!("Cannot canonicalize destination path: {}", dest.display()))?;
if !jdks_dirs.iter().any(|d| canonical_dest.starts_with(d)) {
    bail!("destination '{}' is outside all configured jdks_dirs", canonical_dest.display());
}
```

This pattern appears in: `infra/config.rs`, `core/downloader.rs`, `commands/delete.rs`, `core/jdk_switcher.rs`.

### Archive extraction — server-supplied filenames

Always strip the directory from server-supplied archive filenames:

```rust
// artifact.filename is a server-supplied string — strip to basename only
let file_name = Path::new(&artifact.filename)
    .file_name()
    .context("artifact filename has no file component")?;
```

The pinned deps `tar = "=0.4.45"` and `zip = "=2.3.0"` patch path traversal CVEs in archive extraction.

### Version strings — shell metacharacter rejection

All version arguments pass `validate_version_string()` before use in any file system operation or API call:

```rust
pub(crate) fn validate_version_string(s: &str) -> Result<(), String> {
    if s.is_empty() {
        return Err("version cannot be empty".to_owned());
    }
    if s.len() > 64 {
        return Err("version string too long (max 64 chars)".to_owned());
    }
    if !s.chars().all(|c| c.is_alphanumeric() || "-._".contains(c)) {
        return Err(
            "version contains illegal characters (only alphanumeric, '-', '.', '_' allowed)"
                .to_owned(),
        );
    }
    Ok(())
}
```

### HTTPS enforcement — no HTTP downgrade

All HTTP calls pass `require_https()` before sending:

```rust
fn require_https(url: &str) -> Result<()> {
    if !url.starts_with("https://") {
        bail!("Refusing to download over non-HTTPS URL: {url}");
    }
    Ok(())
}
```

The HTTP client is also configured with `danger_accept_invalid_certs(false)` and `use_rustls_tls()`.

---

## Dependency Security

### cargo audit (required in CI)

```bash
cargo install cargo-audit
cargo audit    # Checks against the RustSec Advisory Database
```

Run on every push that modifies `Cargo.toml` or `Cargo.lock`, and on a daily cron schedule.

### cargo deny (recommended)

```bash
cargo install cargo-deny
cargo deny check
```

`deny.toml` can enforce: vulnerability policy, license allow-list, banned duplicates.

### cargo geiger (unsafe audit)

```bash
cargo install cargo-geiger
cargo geiger   # Reports unsafe usage in the full dependency tree
```

### Version pinning for security-sensitive deps

Use `=x.y.z` exact pins in `Cargo.toml` for deps with known CVE history:

```toml
tar = "=0.4.45"   # RUSTSEC-2026-0067/0068 — path traversal in tar extraction
zip = "=2.3.0"    # CVE-2025-29787 — path traversal in zip extraction
time = "0.3.47"   # RUSTSEC-2026-0009 / CVE-2026-25727 — security pin
```

When updating a pinned dep: verify the new version resolves the advisory, update the comment, run `cargo audit`.

### General dependency hygiene

- `default-features = false` and enable only the features you need — reduces attack surface.
- Prefer well-maintained crates with recent commits and high download counts.
- Review crate source before adding (crates.io → docs.rs / GitHub).
- `Cargo.lock` must be committed — pins exact versions for auditable builds.

---

## Authentication & Authorization

sjvm is a local CLI tool. Current authentication: `GITHUB_TOKEN` environment variable for elevated GitHub API rate limits (optional).

If authentication is extended in the future:
- Use OS-level credential stores (`keyring` crate) rather than plaintext files.
- Validate that config files have appropriate permissions (not world-readable if they contain sensitive paths).
- Never log authentication tokens, even at debug level.

---

## Common Vulnerabilities

### Symlink attacks (TOCTOU)

sjvm manages symlinks, which introduces TOCTOU (time-of-check / time-of-use) risks:

- After resolving a JDK path with `canonicalize()`, verify the target is inside the configured `jdks_dirs` before creating the symlink.
- Never follow symlinks through user-controlled directories to sensitive system paths.
- Use `#[cfg(unix)]` `warn_if_dangerous_path()` to emit warnings when a config-supplied path resolves inside `/etc`, `/bin`, `/usr/bin`, etc.

### Path traversal in archive extraction

- Pinned `tar` and `zip` versions (see above) patch extraction CVEs.
- Strip server-supplied filenames to basename-only before writing to disk.
- Validate the final destination path with `canonicalize()` + `starts_with()` before writing.

### Unsafe code

- `#![deny(unsafe_code)]` is set at crate root in `main.rs`.
- If unsafe is ever needed, every `unsafe` block must have a `// SAFETY: <invariant>` comment explaining why it is sound.
- In Rust 2024 edition, `unsafe_op_in_unsafe_fn` is deny-by-default: calls inside an `unsafe fn` still require their own `unsafe` block.

### Platform directory creation

App directories are created with `0700` permissions (owner-only) on Unix — preventing other users from reading the JDK memory cache or config:

```rust
#[cfg(unix)]
fs::set_permissions(&dir, Permissions::from_mode(0o700))?;
```
