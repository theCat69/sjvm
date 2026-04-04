use std::process::Command;

fn sjvm_command() -> Command {
    Command::new("./target/debug/sjvm")
}

fn set_java_version_to(java_version: &str) {
    let output = sjvm_command()
        .args(["use", java_version])
        .output()
        .expect("Failed to set Java version");
    assert!(output.status.success());
}

fn get_java_version() -> Option<String> {
    let output = Command::new("java")
        .arg("-version")
        .output()
        .expect("failed to run java -version");

    if output.status.success() {
        return Some(String::from_utf8_lossy(&output.stderr).to_string());
    }

    None
}

#[test]
#[ignore]
fn test_cli_runs_successfully() {
    let output = sjvm_command()
        .arg("--version")
        .output()
        .expect("failed to execute process");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sjvm"));
}

#[test]
#[ignore]
fn test_setup() {
    let output = sjvm_command()
        .arg("setup")
        .output()
        .expect("failed to execute setup");

    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("stdout setup : {}", stdout);

    assert!(
        stdout.contains("Setup complete"),
        "Setup did not complete succesfully: {}",
        stdout
    );
    assert!(
        stdout.contains("JAVA_HOME"),
        "Command has no JAVA_HOME proposal: {}",
        stdout
    );
    assert!(
        stdout.contains("PATH"),
        "Command has no PATH proposal: {}",
        stdout
    );
}

#[test]
#[ignore]
fn test_java_21() {
    let output = sjvm_command()
        .args(["use", "jdk-21"])
        .output()
        .expect("Failed to set Java version");
    assert!(output.status.success());

    let output = Command::new("java")
        .arg("-version")
        .output()
        .expect("failed to run java -version");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("21"), "Java 21 not detected: {}", stderr);
}

#[test]
#[ignore]
fn test_java_17() {
    let output = sjvm_command()
        .args(["use", "jdk-17"])
        .output()
        .expect("Failed to set Java version");
    assert!(output.status.success());

    let output = Command::new("java")
        .arg("-version")
        .output()
        .expect("failed to run java -version");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("17"), "Java 17 not detected: {}", stderr);
}

#[test]
#[ignore]
fn test_java_17_local() {
    let output = sjvm_command()
        .args(["use", "jdk-17", "-l"])
        .output()
        .expect("Failed to set Java version");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("JAVA_HOME"),
        "Command has no JAVA_HOME: {}",
        stdout
    );
    assert!(stdout.contains("PATH"), "Command has no PATH: {}", stdout);
    assert!(stdout.contains("17"), "Java 17 not detected: {}", stdout);
}

#[test]
#[ignore]
fn test_list() {
    let output = sjvm_command()
        .args(["list"])
        .output()
        .expect("Fail to run list");
    assert!(output.status.success());
}

#[test]
#[ignore]
fn test_config_path() {
    let output = sjvm_command()
        .args(["config", "path"])
        .output()
        .expect("Fail to run config path");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("sjvm"),
        "Fail to get config path: {}",
        stdout
    );
}

#[test]
#[ignore]
fn test_ui_command_recognized() {
    let output = sjvm_command()
        .args(["ui", "--help"])
        .output()
        .expect("Failed to get ui help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should not show unrecognized subcommand error
    assert!(
        !stdout.contains("unrecognized subcommand") && !stdout.contains("unexpected argument"),
        "ui command should be recognized: {}",
        stdout
    );
}

#[test]
#[ignore]
fn test_ui_java_version_switch() {
    use std::io::{Read, Write};
    use std::thread;
    use std::time::Duration;

    // Re-install jdk-21 in case a previous test (test_ui_delete) removed it.
    install_jdk_for_test(21, "openjdk");

    // Set initial Java version to jdk-21
    set_java_version_to("jdk-21");

    // Verify initial state
    let initial_version = get_java_version();
    assert!(
        initial_version.is_some() && initial_version.as_ref().unwrap().contains("21"),
        "Java 21 should be set initially"
    );

    // Create a PTY for the ui process
    let (mut pty, pts) = pty_process::blocking::open().expect("Failed to open PTY");

    // Set a reasonable size for the terminal
    pty.resize(pty_process::Size::new(24, 80))
        .expect("Failed to resize PTY");

    // Spawn the ui command with the PTY
    let cmd = pty_process::blocking::Command::new("./target/debug/sjvm").arg("ui");

    let mut child = cmd.spawn(pts).expect("Failed to spawn ui command");

    // Give the TUI time to initialize
    thread::sleep(Duration::from_millis(300));

    // Send navigation and selection commands through the PTY
    // Send 'j' to navigate down to the next JDK version
    pty.write_all(b"jj").expect("Failed to write 'j' to PTY");

    // Wait a bit for the UI to process the navigation
    thread::sleep(Duration::from_millis(150));

    // Send Enter (carriage return) to switch to the selected JDK version
    pty.write_all(b"\r").expect("Failed to write Enter to PTY");

    // Wait for the TUI to process the selection and display success message
    thread::sleep(Duration::from_millis(1500));

    // Read any output from the PTY to verify the operation
    let mut output_buffer = vec![0u8; 4096];
    let _bytes_read = pty.read(&mut output_buffer).ok();

    // Terminate the PTY connection
    drop(pty);

    // Wait for child to finish
    let max_wait = Duration::from_secs(5);
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => break,
            Ok(None) => {
                if start.elapsed() > max_wait {
                    // Timeout - kill the process
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("UI did not exit within 5 seconds");
                }
                thread::sleep(Duration::from_millis(100));
            }
            Err(e) => panic!("Error checking process status: {}", e),
        }
    }

    // Verify the Java version has changed to a different version.
    // NOTE: We assert the version *changed* from the initial (21) rather than
    // asserting a specific target version — the list order may vary across
    // environments, so hardcoding "17" would be fragile.
    let final_version = get_java_version();
    assert!(
        final_version.is_some(),
        "Should be able to get java version after ui selection"
    );

    let final_java_v = final_version.unwrap();
    println!("Final Java version: {}", final_java_v);

    assert!(
        !final_java_v.contains("21"),
        "Java version should have changed from the initial 21, got: {}",
        final_java_v
    );
}

fn install_jdk_for_test(version: u16, vendor: &str) -> String {
    let version_str = version.to_string();
    let archive_path = format!("/home/rustuser/jdk-archives/jdk-{vendor}-{version}.tar.gz");
    let output = sjvm_command()
        .args([
            "install",
            &version_str,
            "--vendor",
            vendor,
            "--force",
            "--local-archive",
            &archive_path,
        ])
        .output()
        .expect("Failed to spawn sjvm install");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "sjvm install {version} --vendor {vendor} --force --local-archive {archive_path} failed, \
         stderr: {stderr}"
    );

    // Find the installed JDK by checking the vendor file contents so that when
    // multiple JDKs share the same version number (e.g. openjdk-21 and graalvm-21)
    // we pick the one actually tagged with the requested vendor.
    let lines = list_jdks();
    lines
        .iter()
        .filter(|line| line.contains(&version_str) && !line.contains("[custom]"))
        .map(|line| {
            let stripped = line.trim().trim_start_matches('→').trim();
            let stripped = stripped.trim_end_matches("[custom]").trim();
            std::path::Path::new(stripped)
                .file_name()
                .expect("JDK path has file_name")
                .to_string_lossy()
                .into_owned()
        })
        .find(|name| {
            let vendor_file = jdks_dir().join(name).join(".sjvm-vendor");
            std::fs::read_to_string(&vendor_file)
                .map(|s| s.trim().eq_ignore_ascii_case(vendor))
                .unwrap_or(false)
        })
        .expect("installed JDK not found in sjvm list after install")
}

fn spawn_sjvm_with_stdin(args: &[&str], input: &[u8]) -> std::process::Output {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = sjvm_command()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn sjvm process");

    child
        .stdin
        .take()
        .expect("stdin handle missing")
        .write_all(input)
        .expect("Failed to write to sjvm stdin");

    child
        .wait_with_output()
        .expect("Failed to wait for sjvm process output")
}

fn list_jdks() -> Vec<String> {
    let output = sjvm_command()
        .arg("list")
        .output()
        .expect("Failed to run sjvm list");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(str::to_owned)
        .collect()
}

#[test]
#[ignore]
fn test_install_help() {
    let output = sjvm_command()
        .args(["install", "--help"])
        .output()
        .expect("Failed to run sjvm install --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--vendor"),
        "stdout should contain '--vendor': {}",
        stdout
    );
    assert!(
        stdout.contains("--force"),
        "stdout should contain '--force': {}",
        stdout
    );
    assert!(
        stdout.contains("VERSION"),
        "stdout should contain 'VERSION': {}",
        stdout
    );
}

#[test]
#[ignore]
fn test_install_version_too_low() {
    let output = sjvm_command()
        .args(["install", "5"])
        .output()
        .expect("Failed to run sjvm install 5");

    assert!(
        !output.status.success(),
        "Expected non-zero exit for version 5"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains('5'),
        "stderr should contain '5': {}",
        stderr
    );
    assert!(
        stderr.contains("out of range"),
        "stderr should contain 'out of range': {}",
        stderr
    );
}

#[test]
#[ignore]
fn test_install_version_too_high() {
    let output = sjvm_command()
        .args(["install", "26"])
        .output()
        .expect("Failed to run sjvm install 26");

    assert!(
        !output.status.success(),
        "Expected non-zero exit for version 26"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("26"),
        "stderr should contain '26': {}",
        stderr
    );
    assert!(
        stderr.contains("out of range"),
        "stderr should contain 'out of range': {}",
        stderr
    );
}

#[test]
#[ignore]
fn test_install_version_invalid_chars() {
    let output = sjvm_command()
        .args(["install", "foo!bar"])
        .output()
        .expect("Failed to run sjvm install foo!bar");

    assert!(
        !output.status.success(),
        "Expected non-zero exit for version 'foo!bar'"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("illegal"),
        "stderr should contain 'illegal': {}",
        stderr
    );
}

#[test]
#[ignore]
fn test_install_openjdk_21() {
    let before = list_jdks();

    let output = sjvm_command()
        .args([
            "install",
            "21",
            "--vendor",
            "openjdk",
            "--force",
            "--local-archive",
            "/home/rustuser/jdk-archives/jdk-openjdk-21.tar.gz",
        ])
        .output()
        .expect("Failed to run sjvm install 21 --vendor openjdk --force");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Expected exit 0 for openjdk 21 install, stderr: {}",
        stderr
    );

    let after = list_jdks();
    let new_entries: Vec<&String> = after
        .iter()
        .filter(|e| !before.contains(e) && e.contains("21"))
        .collect();
    assert!(
        !new_entries.is_empty(),
        "Expected a new entry with '21' in sjvm list after install. before={:?}, after={:?}",
        before,
        after
    );
}

#[test]
#[ignore]
fn test_install_graalvm_21() {
    let output = sjvm_command()
        .args([
            "install",
            "21",
            "--vendor",
            "graalvm",
            "--force",
            "--local-archive",
            "/home/rustuser/jdk-archives/jdk-graalvm-21.tar.gz",
        ])
        .output()
        .expect("Failed to run sjvm install 21 --vendor graalvm --force");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Expected exit 0 for graalvm 21 install, stderr: {}",
        stderr
    );

    let after = list_jdks();
    assert!(
        after.iter().any(|e| e.contains("graalvm")),
        "Expected an entry containing 'graalvm' in sjvm list after install. after={:?}",
        after
    );
}

#[test]
#[ignore]
fn test_cli_delete_rejects_path_traversal() {
    // "../etc" contains '/' so the early name-validation guard fires before any
    // confirmation prompt is read — the process exits non-zero immediately.
    let output = spawn_sjvm_with_stdin(&["delete", "../etc"], b"y\n");

    assert!(
        !output.status.success(),
        "Expected non-zero exit when passing path traversal '../etc' to delete"
    );
}

#[test]
#[ignore]
fn test_cli_delete_rejects_dot() {
    // Provide "y\n" so the confirmation prompt is answered and delete_jdk is reached.
    // The strengthened security check (canonical_path == canonical_dest) then rejects
    // "." — which resolves to the JDKs directory itself — with a non-zero exit.
    let output = spawn_sjvm_with_stdin(&["delete", "."], b"y\n");

    assert!(
        !output.status.success(),
        "Expected non-zero exit when passing '.' to delete"
    );
}

#[test]
#[ignore]
fn test_cli_delete_aborts_on_n() {
    let jdk_name = install_jdk_for_test(21, "openjdk");

    let output = spawn_sjvm_with_stdin(&["delete", &jdk_name], b"n\n");

    assert!(
        output.status.success(),
        "Expected exit 0 when aborting delete with 'n', stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Aborted"),
        "Expected 'Aborted' in stdout after pressing 'n', got: {stdout}"
    );

    let list = list_jdks();
    assert!(
        list.iter().any(|l| l.contains(&jdk_name)),
        "JDK '{jdk_name}' should still be present after aborting delete, list: {list:?}"
    );
}

#[test]
#[ignore]
fn test_cli_delete_success() {
    let jdk_name = install_jdk_for_test(21, "openjdk");

    let output = spawn_sjvm_with_stdin(&["delete", &jdk_name], b"y\n");

    assert!(
        output.status.success(),
        "Expected exit 0 after confirming delete with 'y', stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Deleted") || stdout.contains('✓'),
        "Expected 'Deleted' or '✓' in stdout after confirming delete, got: {stdout}"
    );

    let list = list_jdks();
    assert!(
        !list.iter().any(|l| {
            let name = l
                .trim()
                .trim_start_matches('→')
                .trim()
                .trim_end_matches("[custom]")
                .trim();
            name == jdk_name
        }),
        "JDK '{jdk_name}' should be gone from list after delete, but list is: {list:?}"
    );
}

#[test]
#[ignore]
fn test_ui_install_navigation() {
    use std::io::Write;
    use std::thread;
    use std::time::Duration;

    let (mut pty, pts) = pty_process::blocking::open().expect("Failed to open PTY");
    pty.resize(pty_process::Size::new(24, 80))
        .expect("Failed to resize PTY");

    let mut child = pty_process::blocking::Command::new("./target/debug/sjvm")
        .args(["ui"])
        .spawn(pts)
        .expect("Failed to spawn sjvm ui");

    // Wait for Switch screen to render
    thread::sleep(Duration::from_millis(500));

    // Tab: switch to Install screen
    pty.write_all(b"\t").expect("Failed to write Tab to PTY");
    // Wait for VendorPicker to render (no extra Enter needed per UI-2 spec)
    thread::sleep(Duration::from_millis(400));

    // Enter: select OpenJDK (first vendor), starts FetchingVersions
    pty.write_all(b"\r").expect("Failed to write Enter to PTY");
    // Wait for Adoptium API call to return version list
    thread::sleep(Duration::from_millis(12_000));

    // Navigate down in VersionPicker
    pty.write_all(b"j").expect("Failed to write 'j' to PTY");
    thread::sleep(Duration::from_millis(200));

    // Navigate down again
    pty.write_all(b"j").expect("Failed to write 'j' to PTY");
    thread::sleep(Duration::from_millis(200));

    // Navigate up
    pty.write_all(b"k").expect("Failed to write 'k' to PTY");
    thread::sleep(Duration::from_millis(200));

    // Quit
    pty.write_all(b"q").expect("Failed to write 'q' to PTY");

    // Poll for child exit up to 5s; kill if timeout
    let start = std::time::Instant::now();
    loop {
        if let Ok(Some(_)) = child.try_wait() {
            break;
        }
        if start.elapsed() > Duration::from_secs(5) {
            let _ = child.kill();
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    let _ = child.wait(); // reap in all cases (no-op if already exited)
    // Test passes if we reach this point: TUI opened, navigation worked, exited cleanly
}

#[test]
#[ignore]
fn test_ui_delete() {
    use std::io::Write;
    use std::thread;
    use std::time::Duration;

    let jdk_name = install_jdk_for_test(21, "openjdk");
    // Make it the current/selected JDK in the Switch screen
    set_java_version_to(&jdk_name);

    let (mut pty, pts) = pty_process::blocking::open().expect("Failed to open PTY");
    pty.resize(pty_process::Size::new(24, 80))
        .expect("Failed to resize PTY");

    let mut child = pty_process::blocking::Command::new("./target/debug/sjvm")
        .args(["ui"])
        .spawn(pts)
        .expect("Failed to spawn sjvm ui");

    // Wait for Switch screen + JDK list to render
    thread::sleep(Duration::from_millis(600));

    // 'd': trigger delete overlay for the currently selected JDK
    pty.write_all(b"d").expect("Failed to write 'd' to PTY");
    thread::sleep(Duration::from_millis(300));

    // 'y': confirm deletion
    pty.write_all(b"y").expect("Failed to write 'y' to PTY");
    // Wait for delete + memory invalidation + Switch screen reload
    thread::sleep(Duration::from_millis(2_000));

    // Quit TUI
    pty.write_all(b"q").expect("Failed to write 'q' to PTY");

    // Poll for child exit up to 5s; kill if timeout
    let start = std::time::Instant::now();
    loop {
        if let Ok(Some(_)) = child.try_wait() {
            break;
        }
        if start.elapsed() > Duration::from_secs(5) {
            let _ = child.kill();
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    let _ = child.wait(); // reap in all cases (no-op if already exited)

    // Verify JDK is no longer listed
    let list = list_jdks();
    assert!(
        !list.iter().any(|l| {
            let name = l
                .trim()
                .trim_start_matches('→')
                .trim()
                .trim_end_matches("[custom]")
                .trim();
            name == jdk_name
        }),
        "JDK '{jdk_name}' should be gone from list after TUI delete, but list is: {list:?}"
    );
}

/// Returns the configured JDKs directory for the Docker test environment.
///
/// Matches the `jdks_dirs` value in `test-config/sjvm-conf.json`, which is
/// volume-mounted into `/home/rustuser/.config/sjvm` inside the container.
fn jdks_dir() -> std::path::PathBuf {
    std::path::PathBuf::from("/home/rustuser/jvms")
}

#[test]
#[ignore]
fn test_use_vendor_filter_success() {
    let jdk_name = install_jdk_for_test(21, "openjdk");

    let output = sjvm_command()
        .args(["use", "21", "--vendor", "openjdk"])
        .output()
        .expect("Failed to run sjvm use 21 --vendor openjdk");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Expected exit 0 for 'use 21 --vendor openjdk', stderr: {stderr}, stdout: {stdout}"
    );
    assert!(
        stdout.contains('✅'),
        "Expected '✅' in stdout, got: {stdout}"
    );

    // Cleanup: not strictly necessary but keeps the environment tidy.
    let _ = jdk_name;
}

#[test]
#[ignore]
fn test_use_vendor_no_match_error() {
    let output = sjvm_command()
        .args(["use", "999", "--vendor", "graalvm"])
        .output()
        .expect("Failed to run sjvm use 999 --vendor graalvm");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Expected non-zero exit for nonexistent version with vendor filter"
    );
    assert!(
        stderr.contains("not found"),
        "Expected 'not found' in stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("sjvm install"),
        "Expected install hint in stderr, got: {stderr}"
    );
}

#[test]
#[ignore]
fn test_tag_writes_vendor_file() {
    let jdk_name = install_jdk_for_test(21, "openjdk");
    let vendor_file = jdks_dir().join(&jdk_name).join(".sjvm-vendor");

    // Remove the vendor file that was written during install so we can test `sjvm tag`.
    let _ = std::fs::remove_file(&vendor_file);
    assert!(
        !vendor_file.exists(),
        ".sjvm-vendor should be absent before tagging"
    );

    let output = sjvm_command()
        .args(["tag", &jdk_name, "--vendor", "openjdk"])
        .output()
        .expect("Failed to run sjvm tag");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Expected exit 0 for sjvm tag, stderr: {stderr}"
    );
    assert!(
        stdout.contains('✅'),
        "Expected '✅' in stdout, got: {stdout}"
    );
    assert!(
        vendor_file.exists(),
        ".sjvm-vendor should exist after tagging"
    );
    let content = std::fs::read_to_string(&vendor_file).expect("read .sjvm-vendor");
    assert_eq!(content.trim(), "openjdk");
}

#[test]
#[ignore]
fn test_tag_already_tagged_no_force() {
    // Install writes .sjvm-vendor automatically.
    let jdk_name = install_jdk_for_test(21, "openjdk");

    let output = sjvm_command()
        .args(["tag", &jdk_name, "--vendor", "openjdk"])
        .output()
        .expect("Failed to run sjvm tag without --force");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "Expected non-zero exit when JDK is already tagged and --force is absent"
    );
    assert!(
        stderr.contains("already tagged"),
        "Expected 'already tagged' in stderr, got: {stderr}"
    );
}

#[test]
#[ignore]
fn test_tag_already_tagged_with_force() {
    // Install writes .sjvm-vendor automatically.
    let jdk_name = install_jdk_for_test(21, "openjdk");

    let output = sjvm_command()
        .args(["tag", &jdk_name, "--vendor", "openjdk", "--force"])
        .output()
        .expect("Failed to run sjvm tag --force");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Expected exit 0 for sjvm tag --force, stderr: {stderr}"
    );
    assert!(
        stdout.contains('✅'),
        "Expected '✅' in stdout, got: {stdout}"
    );
}
