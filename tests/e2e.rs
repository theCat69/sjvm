use std::process::Command;

#[test]
fn test_cli_runs_successfully() {
    let output = Command::new("./target/release/sjvm")
        .arg("--version")
        .output()
        .expect("failed to execute process");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sjvm"));
}

#[test]
fn test_setup() {
    let output = Command::new("./target/release/sjvm")
        .arg("setup")
        .output()
        .expect("failed to execute setup");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Setup complete"),
        "Setup did not complete succesfully: {}",
        stdout
    );
    assert!(
        stdout.contains("JAVA_HOME"),
        "Command has not JAVA_HOME proposal: {}",
        stdout
    );
    assert!(
        stdout.contains("PATH"),
        "Command has not PATH proposal: {}",
        stdout
    );
}

#[test]
fn test_java_21() {
    let status = Command::new("./target/release/sjvm")
        .args(["use", "jdk-21"])
        .status()
        .expect("Failed to set Java version");
    assert!(status.success());

    let output = Command::new("java")
        .arg("-version")
        .output()
        .expect("failed to run java -version");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("21"), "Java 21 not detected: {}", stderr);
}

#[test]
fn test_java_17() {
    let status = Command::new("./target/release/sjvm")
        .args(["use", "jdk-17"])
        .status()
        .expect("Failed to set Java version");
    assert!(status.success());

    let output = Command::new("java")
        .arg("-version")
        .output()
        .expect("failed to run java -version");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("17"), "Java 17 not detected: {}", stderr);
}

#[test]
fn test_java_17_local() {
    let output = Command::new("./target/release/sjvm")
        .args(["use", "jdk-17", "-l"])
        .output()
        .expect("Failed to set Java version");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("set JAVA_HOME"),
        "Command has not JAVA_HOME set: {}",
        stdout
    );
    assert!(
        stdout.contains("set PATH"),
        "Command has not PATH set: {}",
        stdout
    );
    assert!(stdout.contains("17"), "Java 17 not detected: {}", stdout);
}

#[test]
fn test_list() {
    let status = Command::new("./target/release/sjvm")
        .args(["list"])
        .status()
        .expect("Fail to run list");
    assert!(status.success());
}

#[test]
fn test_config_path() {
    let output = Command::new("./target/release/sjvm")
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
