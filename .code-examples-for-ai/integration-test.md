<!-- Demonstrates integration test pattern for CLI behavior in tests/e2e.rs -->

```rust
use std::process::Command;

fn sjvm_command() -> Command {
    Command::new("./target/debug/sjvm")
}

#[test]
#[ignore] // docker/environment-dependent end-to-end flow
fn test_install_help() {
    let output = sjvm_command()
        .args(["install", "--help"])
        .output()
        .expect("Failed to run sjvm install --help");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--vendor"), "stdout should contain '--vendor': {}", stdout);
    assert!(stdout.contains("--force"), "stdout should contain '--force': {}", stdout);
    assert!(stdout.contains("VERSION"), "stdout should contain 'VERSION': {}", stdout);
}

#[test]
#[ignore]
fn test_install_version_too_low() {
    let output = sjvm_command()
        .args(["install", "5"])
        .output()
        .expect("Failed to run sjvm install 5");

    assert!(!output.status.success(), "Expected non-zero exit for version 5");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("out of range"), "stderr should contain 'out of range': {}", stderr);
}
```
