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

    // Start the interactive UI process - it will fail due to no terminal
    // Start the interactive UI process with proper terminal environment
    let mut child = sjvm_command()
        .arg("interactive")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start interactive command");
    // Send 'q' as a raw character to quit the interactive UI immediately
    if let Some(stdin) = child.stdin.as_mut() {
        use std::io::Write;
        println!("got some stdin");
        stdin.write_all(b"q").expect("Failed to write 'q' to stdin");
        stdin.flush().expect("Failed to flush stdin");
    }

    // Wait for the process to finish and bind to it
    let output = child
        .wait_with_output()
        .expect("Failed to wait for interactive command");

    let stderr = String::from_utf8_lossy(&output.stderr);

    // println!("stdout: {:?}", String::from_utf8_lossy(&output.stdout));
    // println!("sterr: {:?}", stderr);

    assert!(
        !stderr.contains("Error") && !stderr.contains("error"),
        "Interactive UI should open and quit gracefully, {}",
        stderr
    );

    // Should exit with error code 1 (expected for terminal error)
    // assert_eq!(output.status.code(), Some(1));
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
