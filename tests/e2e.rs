use std::process::Command;

fn sjvm_command() -> Command {
    let mut cmd = Command::new("./target/release/sjvm");
    cmd.env("JAVA_HOME", "/home/rustuser/.java/current")
        .env("PATH", "/home/rustuser/.java/current/bin:$PATH");
    cmd
}

#[test]
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
fn test_debug() {
    let output = Command::new("ls")
        .arg("-ltr")
        .arg("/home/rustuser/.java")
        .output()
        .expect("failed to execute process");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("stdout debug : {}", stdout);

    let output = Command::new("ls")
        .arg("-ltr")
        .arg("/home/rustuser/.config/sjvm")
        .output()
        .expect("failed to execute process");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("stdout debug : {}", stdout);

    let output = Command::new("cat")
        .arg("/home/rustuser/.config/sjvm/sjvm-config.json")
        .output()
        .expect("failed to execute process");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("stdout debug : {}", stdout);
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
fn test_java_21() {
    let status = sjvm_command()
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
    let status = sjvm_command()
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
    let output = sjvm_command()
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
    let status = sjvm_command()
        .args(["list"])
        .status()
        .expect("Fail to run list");
    assert!(status.success());
}

#[test]
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
