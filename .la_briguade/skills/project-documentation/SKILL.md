---
name: project-documentation
description: Project-specific documentation standards for code, README, API docs, and changelog
agents:
  - coder
  - reviewer
---

## Code Documentation

- Use `//!` for module/crate-level docs.
- Use `///` for public and crate-visible API surface where behavior contracts matter.
- Include `# Errors` sections on `Result`-returning APIs.
- Keep examples compile-ready and aligned with real command/API behavior.

## README Format

README should cover:

- What sjvm does (JDK manager via symlink switching)
- Install/build instructions
- Core commands (`setup`, `use`, `list`, `install`, `delete`, `versions`, `tag`, optional `ui`)
- Feature-flag note for TUI (`--features ui`)
- Security and configuration highlights

## API Documentation

- Generate docs with `cargo doc --no-deps`.
- Prefer concise docs that explain intent, invariants, and failure modes.
- Keep clap help text in sync with command behavior using field and enum doc comments.

## Changelog

- Maintain `CHANGELOG.md` for user-visible changes.
- Record notable CLI behavior, security hardening, and dependency pin changes.
- Keep version entries aligned with release tags and Cargo versioning.
