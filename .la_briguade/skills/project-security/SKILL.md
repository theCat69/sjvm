---
name: project-security
description: Project-specific security guidelines for secrets, input validation, dependencies, auth, and common vulnerabilities
agents:
  - coder
  - reviewer
  - security-reviewer
---

## Secrets Management

- Never commit secrets (`.env`, tokens, credentials).
- Never embed secrets in `anyhow` context strings or user-facing errors.
- Keep `.env` ignored; prefer environment variables for sensitive runtime values.

## Input Validation

- Validate CLI input at parse boundaries (clap validators/value enums).
- Reject unsafe path patterns and prevent traversal (`..`, invalid components).
- For archive installs, sanitize archive-derived names before path construction.
- Ensure destination paths remain direct children of configured JDK roots.

## Dependency Security

- Keep `Cargo.lock` committed (binary crate reproducibility).
- Run `cargo audit` in CI (already configured).
- Prefer minimal dependency feature sets (`default-features = false` where possible).
- Use pinned safe versions where advisories require exact versions (e.g., archive crates).
- Add `cargo deny` in policy pipelines when available for license/source/advisory controls.

## Authentication & Authorization

- Current domain is local CLI JDK management; no built-in auth layer.
- If remote/auth features are added later, store secrets in OS-backed secure storage and never log sensitive values.

## Common Vulnerabilities

- **Path traversal**: validate extraction and destination paths before filesystem writes.
- **Archive extraction attacks**: rely on patched `tar`/`zip` behavior and enforce additional destination checks.
- **Symlink attacks / TOCTOU**: canonicalize trusted roots and verify operations target expected directories.
- **Unsafe transport**: enforce HTTPS for remote metadata/artifact endpoints.
- **Error leakage**: keep error messages actionable without exposing sensitive internals.
