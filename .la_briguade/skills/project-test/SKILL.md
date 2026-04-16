---
name: project-test
description: Project-specific testing guidelines, test framework conventions, patterns, and coverage requirements
agents:
  - coder
  - reviewer
  - builder
---

## Test Framework

- Native Rust test framework (`#[test]`, assertions, module-local tests).
- Clap parser tests via `Cli::try_parse_from(...)` (avoid `Cli::parse()` in tests).
- Optional TUI tests use ratatui test backend patterns where applicable.
- E2E workflow is Docker/container oriented and mostly marked `#[ignore]`.

## Test Location & File Naming

- Unit tests are colocated at file bottom in `#[cfg(test)] mod tests`.
- Integration tests live in `tests/`, notably `tests/e2e.rs`.
- Test names use descriptive `test_<behavior>` style.

## Writing Tests

- Test behavior and contracts, not implementation trivia.
- Prefer deterministic pure-function testing for core parsers/validators.
- For fallible flows, assert both failure presence and useful error content.
- For clap commands, test defaults, invalid values, and subcommand parsing.

## Mocking & Fixtures

- Prefer explicit fixtures/helpers over heavy mocking.
- Existing E2E helper style in `tests/e2e.rs` uses command wrappers and environment-specific local archives.
- Keep side effects isolated and cleanup explicit when tests create filesystem state.

## Coverage Requirements

- No hard percentage gate currently.
- Prioritize coverage for:
  - input validation
  - archive/path security checks
  - CLI parsing and error messaging
  - JDK resolution and switching behavior

## Running Tests

- Main test runs:
  - `cargo test --all-features`
- E2E-specific run (ignored tests):
  - `cargo test --all-features --test e2e -- --ignored`

Recommended quality gate before merge:

- `cargo fmt --check`
- `cargo clippy --all-features -- -D warnings`
- `cargo test --all-features`
