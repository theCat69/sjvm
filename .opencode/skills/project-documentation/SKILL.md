---
name: project-documentation
description: Project-specific documentation standards for code, README, API docs, and changelog
---

# Project Documentation Guidelines — sjvm

Documentation standards for **sjvm** — a Rust 2024 edition CLI.

---

## Code Documentation

### Rustdoc format

All `pub` and `pub(crate)` items must have doc comments:
- `///` for items (functions, structs, enums, fields, methods)
- `//!` for module-level or crate-level documentation (top of `lib.rs`/`main.rs`)

```rust
//! JDK artifact catalog — resolves download URLs for OpenJDK (Adoptium) and GraalVM CE.
//!
//! Pure parse functions are separated from HTTP calls so that unit tests never need
//! a live network connection.

/// Resolves download metadata by querying the vendor's API over HTTPS.
///
/// # Errors
/// Returns an error if the HTTP request fails or the response contains unexpected JSON structure.
///
/// # Panics
/// Does not panic — all errors are propagated as `anyhow::Result`.
pub(crate) fn resolve_artifact(vendor: &Vendor, version: u16, os: &str) -> Result<ArtifactInfo> {
    // ...
}
```

### Rustdoc standard sections

Use these standard sections in doc comments where applicable:

| Section | When to use |
|---------|-------------|
| `# Errors` | Every `Result`-returning function — describe what causes `Err` |
| `# Panics` | Any function that can panic — describe the conditions |
| `# Safety` | Every `unsafe fn` — list all caller invariants |
| `# Examples` | Public functions — show a realistic usage snippet |

### Doc tests

Code blocks in `///` comments are compiled and run by `cargo test --doc`:

```rust
/// Parses a JDK version string.
///
/// # Examples
///
/// ```
/// let result = parse_version("temurin-17-jdk");
/// assert!(result.is_some());
/// ```
pub fn parse_version(dir_name: &str) -> Option<String> { ... }
```

Keep doc test examples minimal, compilable, and use `?` not `.unwrap()` where possible.

### Inline comments

- Use `//` inline comments to explain **why**, not **what** — code should be self-explanatory for the *what*.
- Security guards and non-obvious safety decisions must have comments:
  ```rust
  auth_value.set_sensitive(true);  // Redacted in logs/debug output — token must never appear in traces
  ```
- CVE references for pinned dependencies:
  ```toml
  tar = "=0.4.45"   # RUSTSEC-2026-0067/0068 — path traversal in tar extraction
  ```

---

## README Format

`README.md` at repository root should include:

1. **Project name and one-line description** — what sjvm does.
2. **Badges** (optional) — CI status, Rust version, license.
3. **Quick start** — install and first-use commands.
4. **Usage** — all subcommands with examples:
   ```
   sjvm setup
   sjvm use 17
   sjvm list
   sjvm install 21
   sjvm ui          # (requires --features ui)
   ```
5. **Configuration** — `sjvm-conf.json` location and schema.
6. **Feature flags** — table of available features (`ui`).
7. **Building from source** — reference `building-guidelines.md` or inline commands.
8. **License**.

---

## API Documentation

Generate and review HTML docs locally:

```bash
cargo doc --no-deps --open
```

- `--no-deps` skips dependency documentation (faster, cleaner output).
- All `pub` and `pub(crate)` items should render without missing-docs warnings.
- Use `#[doc(alias = "..")]` to improve search discoverability for items with non-obvious names.
- Use `pub(crate)` (not `pub`) for items that don't need to cross crate boundaries — keeps the rendered doc surface minimal.

---

## Changelog

Maintain a `CHANGELOG.md` at repository root following the [Keep a Changelog](https://keepachangelog.com/) format:

```markdown
# Changelog

## [Unreleased]

### Added
- ...

## [0.2.0] - 2026-03-01

### Added
- `sjvm ui` subcommand (optional TUI via `--features ui`)

### Security
- Pinned tar to 0.4.45 (RUSTSEC-2026-0067/0068)
- Pinned zip to 2.3.0 (CVE-2025-29787)
```

Standard sections: `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, `Security`.

Rules:
- Every PR that changes user-visible behavior must update `## [Unreleased]`.
- Security fixes go in the `Security` section regardless of whether they also changed behavior.
- Annotated git tag for every release: `git tag -a v0.2.0 -m "Release 0.2.0"`.

---

## Cargo.toml Metadata

Required fields:

```toml
[package]
name = "sjvm"
version = "0.2.0"
edition = "2024"
rust-version = "1.88"      # MSRV — enforced at build time
description = "..."
license = "MIT"
repository = "https://github.com/..."
```

Recommended: `keywords` (max 5), `categories` (max 5), `authors`.
