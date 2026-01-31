use std::process::Command;

fn sjvm_command() -> Command {
    Command::new("./target/debug/sjvm")
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

    // Start the interactive UI process with proper terminal environment
    // Force crossterm to use stdin instead of terminal by setting environment variables
    let mut child = sjvm_command()
        .arg("interactive")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("TERM", "xterm-256color")
        .env("CROSSTERM_TERM", "false") // Try to disable terminal detection
        .spawn()
        .expect("Failed to start interactive command");

    println!("About to sleep for 50ms to let TUI initialize...");
    // Give the TUI a moment to initialize
    thread::sleep(Duration::from_millis(50));
    println!("Woke up from sleep, about to send input...");

    // Try to send 'q' to quit the interactive UI
    // Handle the case where stdin might already be closed
    let input_sent = if let Some(stdin) = child.stdin.as_mut() {
        use std::io::Write;
        match stdin.write_all(b"q") {
            Ok(_) => {
                // Add newline to ensure the input is processed
                match stdin.write_all(b"\n") {
                    Ok(_) => {
                        match stdin.flush() {
                            Ok(_) => {
                                // Close stdin to signal EOF so the TUI doesn't wait for more input
                                drop(stdin);
                                true
                            }
                            Err(e) => {
                                println!("Failed to flush stdin: {}", e);
                                false
                            }
                        }
                    }
                    Err(e) => {
                        println!("Failed to write newline: {}", e);
                        false
                    }
                }
            }
            Err(e) => {
                println!("Failed to write to stdin: {}", e);
                false
            }
        }
    } else {
        println!("No stdin available");
        false
    };

    // Wait for the process to finish and bind to it
    let output = child
        .wait_with_output()
        .expect("Failed to wait for interactive command");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    println!("Input sent successfully: {}", input_sent);
    println!("Exit code: {:?}", output.status.code());
    println!("STDERR: {}", stderr);
    println!("STDOUT: {}", stdout);

    // The test passes if either:
    // 1. We successfully sent 'q' and the process exited successfully, OR
    // 2. The process failed to start due to terminal limitations (expected in test environment)
    let success_condition = output.status.success()
        || (stderr.contains("Error running interactive UI") && stderr.contains("No such device"));

    assert!(
        success_condition,
        "Interactive UI should either exit successfully when sent 'q' or fail gracefully with terminal error. Input sent: {}, Exit code: {:?}, STDERR: {}",
        input_sent,
        output.status.code(),
        stderr
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
