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
    let output = sjvm_command()
        .arg("--version")
        .output()
        .expect("failed to execute process");

    if output.status.success() {
        return Some(String::from_utf8_lossy(&output.stdout).to_string());
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
fn test_interactive_command_recognized() {
    let output = sjvm_command()
        .args(["interactive", "--help"])
        .output()
        .expect("Failed to get interactive help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should not show unrecognized subcommand error
    assert!(
        !stdout.contains("unrecognized subcommand") && !stdout.contains("unexpected argument"),
        "Interactive command should be recognized: {}",
        stdout
    );
}

#[test]
#[ignore]
fn test_interactive_ui_opens_and_quits() {
    use std::process::Stdio;
    use std::thread;
    use std::time::Duration;

    set_java_version_to("jdk-21");

    // Start the interactive UI process
    let mut child = sjvm_command()
        .arg("interactive")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start interactive command");

    // Give the TUI time to initialize
    thread::sleep(Duration::from_millis(100));

    // Try to send 'q' to quit
    if let Some(stdin) = child.stdin.as_mut() {
        use std::io::Write;
        let _ = stdin.write_all(b"j");
        let _ = stdin.write_all(b"\n");
        // let _ = stdin.write_all(b"q");
        let _ = stdin.flush();
    }

    // Wait a bit to see if process quits on its own
    thread::sleep(Duration::from_millis(500));

    // Check if process is still running, if so kill it
    let _exit_status = match child.try_wait() {
        Ok(Some(status)) => status,
        Ok(None) => {
            // Process is still running, kill it
            child.kill().expect("Failed to kill process");
            child.wait().expect("Failed to wait after kill")
        }
        Err(e) => panic!("Error checking process status: {}", e),
    };

    // Get output to check for errors
    let output = child
        .wait_with_output()
        .expect("Failed to get process output");

    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("{}", stderr);

    let java_v_opt = get_java_version();
    assert!(java_v_opt.is_some(), "Should be able to get java version");
    let java_v = java_v_opt.unwrap();
    assert!(java_v.contains("17"), "Java 17 not detected: {}", java_v);
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
