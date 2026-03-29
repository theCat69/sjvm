# Documentation Guidelines

Documentation standards for **sjvm** — a Rust 2024 edition CLI.

---

## Code Documentation

### Rustdoc format

All public items (`pub`, `pub(crate)`) must have doc comments. Use `///` for items and `//!` for modules:

```rust
//! Module-level doc comment (inner doc comment) at the top of the file.
//! Describes the module's overall purpose.

/// Returns the resolved path to the active JDK symlink.
///
/// # Errors
///
/// Returns `Err` if the platform config directory cannot be determined
/// or if the config file is malformed JSON.
pub fn symlink_path() -> anyhow::Result<PathBuf> { ... }
```

### Rustdoc sections

Use these standard sections in doc comments where applicable:

| Section | When to use |
|---------|-------------|
| `# Examples` | For any public function — show a realistic usage snippet |
| `# Errors` | For `Result`-returning functions — describe what causes `Err` |
| `# Panics` | For functions that can panic — describe the conditions |
| `# Safety` | For `unsafe fn` — document the invariants callers must uphold |

### Doc tests

Code blocks in `///` doc comments are compiled and run by `cargo test --doc`:

```rust
/// Parses a JDK version string.
///
/// # Examples
///
/// ```
/// let version = parse_version("java-17-openjdk-amd64").unwrap();
/// assert_eq!(version, "17");
/// ```
pub fn parse_version(dir_name: &str) -> Option<String> { ... }
```

- Keep doc test examples minimal and runnable.
- Use `# use sjvm::...;` hidden imports if needed for context.

### Enforcing doc coverage

Add this lint to `main.rs` or `lib.rs` to warn on missing docs:

```rust
#![warn(missing_docs)]
```

---

## README Format

`Readme.md` (at repository root) should include:

1. **Project name and one-line description** — what sjvm does.
2. **Badges** (optional) — build status, Rust version, license.
3. **Quick start** — install and first-use commands.
4. **Usage** — all subcommands with examples:
   ```
   sjvm setup
   sjvm use 17
   sjvm list
   sjvm ui   # (with --features ui)
   ```
5. **Configuration** — `sjvm-conf.json` location and schema.
6. **Building from source** — point to `building-guidelines.md` or inline commands.
7. **License**.

---

## API Documentation

Generate and review HTML docs locally before publishing:

```bash
cargo doc --no-deps --open
```

- `--no-deps` skips documentation for dependencies (faster, cleaner).
- All `pub` and `pub(crate)` items should render without missing-docs warnings.
- Use `#[doc(alias = "..")]` to improve search discoverability for items with non-obvious names.

---

## Changelog

Maintain a `CHANGELOG.md` at the repository root following the [Keep a Changelog](https://keepachangelog.com/) format:

```markdown
# Changelog

## [Unreleased]

### Added
- ...

## [0.2.0] - 2026-03-01

### Added
- `sjvm ui` subcommand (optional TUI via `--features ui`)

### Changed
- ...

### Fixed
- ...
```

- Reference the changelog in `Cargo.toml` when publishing to crates.io.
- Every PR that changes user-visible behavior should update `## [Unreleased]`.
