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
    pty.write_all(b"j").expect("Failed to write 'j' to PTY");

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

    // Verify the Java version has changed to a different version
    let final_version = get_java_version();
    assert!(
        final_version.is_some(),
        "Should be able to get java version after ui selection"
    );

    let final_java_v = final_version.unwrap();
    println!("Final Java version: {}", final_java_v);

    // The version should have changed from 21 to something else (17 or 11)
    assert!(
        final_java_v.contains("17"),
        "Java version should have changed from 21 to 17, got: {}",
        final_java_v
    );
}

// #[test]
// fn test_debug() {
//     let output = Command::new("ls")
//         .arg("-ltr")
//         .arg("/home/rustuser/.java")
//         .output()
//         .expect("failed to execute process");
//
//     assert!(output.status.success());
//     let stdout = String::from_utf8_lossy(&output.stdout);
//     println!("stdout debug : {}", stdout);
//
//     let output = sjvm_command()
//         .arg("config")
//         .arg("path")
//         .output()
//         .expect("failed to execute process");
//
//     assert!(output.status.success());
//     let stdout = String::from_utf8_lossy(&output.stdout);
//     println!("stdout debug : {}", stdout);
//
//     let output = Command::new("ls")
//         .arg("-ltr")
//         .arg("/home/rustuser/.config/sjvm")
//         .output()
//         .expect("failed to execute process");
//
//     // assert!(output.status.success());
//     let stdout = String::from_utf8_lossy(&output.stdout);
//     println!("stdout debug : {}", stdout);
//
//     let output = Command::new("ls")
//         .arg("-ltr")
//         .arg("/home/rustuser/jvms")
//         .output()
//         .expect("failed to execute process");
//
//     // assert!(output.status.success());
//     let stdout = String::from_utf8_lossy(&output.stdout);
//     println!("stdout debug : {}", stdout);
//
//     let output = Command::new("cat")
//         .arg("/home/rustuser/.config/sjvm/sjvm-config.json")
//         .output()
//         .expect("failed to execute process");
//
//     // assert!(output.status.success());
//     let stdout = String::from_utf8_lossy(&output.stdout);
//     println!("stdout debug : {}", stdout);
// }
