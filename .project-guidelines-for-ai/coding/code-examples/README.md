# Code Examples

This folder contains representative code snippets and patterns for AI agents to follow when implementing features in **sjvm**.

## Purpose

When an AI agent generates code for this project, it should refer to the examples in this folder to match the project's established patterns, idioms, and style — rather than inventing new approaches.

## How to Use

1. **Before writing new code**, check whether an example exists that demonstrates the relevant pattern.
2. **Follow the example closely** — same import style, same error handling idiom, same naming convention.
3. If no example exists for a new pattern, **add one** after the implementation is reviewed and merged.

## Expected Example Types

Given the detected tech stack, the following example files are recommended:

| File | Covers |
|------|--------|
| `clap-subcommand.md` | Defining `Cli` / `Commands` with clap derive; `try_parse_from` in tests |
| `anyhow-error-handling.md` | `with_context`, `bail!`, `ensure!`, `?` propagation |
| `oncelock-singleton.md` | `OnceLock<T>` lazy-init global state pattern |
| `platform-symlinks.md` | `#[cfg(target_os)]` guards for symlink creation/removal |
| `ratatui-widget.md` | `impl Widget for &Foo` pattern; `TestBackend` for unit tests |
| `bincode-cache.md` | `bincode` 2.0 `Encode`/`Decode` derive; read/write binary cache |
| `unit-test-patterns.md` | `#[cfg(test)]`, `use super::*`, `anyhow::Result<()>` return type in tests |

## Languages & Frameworks

- **Rust** (2024 edition, MSRV 1.85)
- **clap** 4.5 — derive API (`Parser`, `Subcommand`, `Args`)
- **anyhow** 1.0 — error propagation with context
- **serde** / **serde_json** — JSON config serialization
- **bincode** 2.0 — binary cache serialization
- **directories** 6 — cross-platform path resolution
- **walkdir** 2.5 — directory traversal
- **ratatui** 0.30 + **crossterm** 0.29 — optional TUI (feature `ui`)

## Adding New Examples

1. Create a Markdown file in this directory named after the pattern.
2. Use a fenced ` ```rust ` code block.
3. Include a brief explanation of **why** this pattern is used, not just **what** it does.
4. Keep examples minimal but realistic — copy from actual production code in `src/` where possible.
