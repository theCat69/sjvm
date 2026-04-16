---
name: project-coding
description: Project-specific coding guidelines, naming conventions, architecture patterns, and code examples
agents:
  - coder
  - reviewer
  - architect
  - feature-designer
  - feature-reviewer
  - planner
  - ask
  - builder
  - orchestrator
---

## Code Style

- Rust edition: **2024**, MSRV **1.88**.
- Keep formatting rustfmt-clean (`cargo fmt --check` in CI).
- Keep Clippy warning-free (`cargo clippy -- -D warnings`, and `--all-features` in CI).
- Use standard Rust style: 4-space indentation, snake_case modules/functions, CamelCase types.
- Keep command handlers thin; dispatch in `src/main.rs`, business logic in `src/commands/`, `src/core/`, `src/infra/`.

## Naming Conventions

- Modules/functions/locals/files: `snake_case`.
- Structs/enums/traits/enum variants: `UpperCamelCase`.
- Constants/statics: `SCREAMING_SNAKE_CASE`.
- Prefer intention-revealing names (`install_from_local_archive`, `validate_dest_within_jdks_dir`).

## Import Ordering

Use three groups, separated by blank lines:

1. `std` imports
2. Third-party crates
3. `crate::...` imports

Prefer grouped `std` imports where it improves readability (for example `use std::{fs, path::{Path, PathBuf}};`).

## Error Handling

- Use `anyhow::Result<T>` in application code.
- Propagate with `?` and attach context with `.context(...)` / `.with_context(...)`.
- Use `bail!` and `ensure!` for validation and early returns.
- Avoid `unwrap`/`expect` in production paths.
- Keep user-facing command failures printed to stderr with non-zero exit from `main` dispatch.

## Patterns & Architecture

- Architecture split:
  - `src/commands/`: CLI-facing command handlers
  - `src/core/`: domain logic (catalog, downloader, switching)
  - `src/infra/`: filesystem/http/config/cache helpers
- CLI parsing uses **clap derive API** (`Parser`, `Subcommand`, `ValueEnum`) with validator hooks.
- Optional UI is feature-gated (`#[cfg(feature = "ui")]`) and uses ratatui + crossterm.
- TUI follows immediate-mode render loop pattern: poll events, mutate state, redraw frame.
- Security-sensitive flows (archive extraction, destination validation, HTTPS-only fetches) are validated in core logic.

## Code Examples

See `.code-examples-for-ai/` for project-grounded examples.

- Existing examples include:
  - `anyhow-error-handling.md`
  - `clap-subcommand.md`
  - `bincode-cache.md`
  - `platform-symlinks.md`
  - `unit-test-patterns.md`
  - `tui-widget.md`
  - `archive-extraction.md`
  - `integration-test.md`
