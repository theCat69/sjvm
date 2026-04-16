---
name: project-build
description: Project-specific build commands, prerequisites, environment setup, and CI/CD pipeline
agents:
  - coder
  - builder
  - orchestrator
---

## Prerequisites

- Rust stable toolchain, with **MSRV 1.88** and edition 2024.
- Cargo available from Rust toolchain.
- `rustfmt` and `clippy` components installed.
- Docker required only for containerized E2E workflows.

## Environment Setup

- Verify toolchain:
  - `rustc --version` (must be >= 1.88)
  - `cargo --version`
- This repo is a single binary crate (`sjvm`) with `Cargo.lock` committed.
- Optional `ui` feature enables ratatui/crossterm code paths.

## Build Commands

- Fast checks:
  - `cargo check`
  - `cargo check --all-features`
- Linting:
  - `cargo clippy -- -D warnings`
  - `cargo clippy --all-features -- -D warnings`
- Formatting:
  - `cargo fmt --check`
- Release build:
  - `cargo build --release`

Use `--all-features` when validating changes that may affect optional UI code.

## Development Server

This project is a CLI binary and has no long-running dev server.

- Local run examples:
  - `cargo run -- --help`
  - `cargo run -- list`
  - `cargo run --features ui -- ui`

## CI/CD Pipeline

GitHub Actions workflow: `.github/workflows/ci.yml`

- Stable job runs:
  - `cargo fmt --check`
  - `cargo check`
  - `cargo check --all-features`
  - `cargo clippy -- -D warnings`
  - `cargo clippy --all-features -- -D warnings`
  - `cargo test --all-features`
  - `cargo build --release`
- MSRV job validates Rust 1.88 compatibility (`cargo check --all-features`, `cargo test --all-features`).
- Security job runs `cargo audit`.
