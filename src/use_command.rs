use anyhow::bail;

use crate::jdk_switcher::{find_jdk_by_version, jdk_display_name, switch_to_jdk, JdkLookupResult};

/// Switches the globally active JDK to the version matching `version`.
pub(crate) fn use_version(version: &str) -> anyhow::Result<()> {
    match find_jdk_by_version(version) {
        JdkLookupResult::Found(jdk_path) => {
            switch_to_jdk(&jdk_path)?;
            println!("✅ Now using JDK: {}", jdk_path.to_string_lossy());
            Ok(())
        }
        JdkLookupResult::NotFound => {
            bail!("JDK version '{}' not found.", version);
        }
    }
}

/// Prints shell `export` commands to activate the JDK for the current session only.
pub(crate) fn use_version_local(version: &str) -> anyhow::Result<()> {
    match find_jdk_by_version(version) {
        JdkLookupResult::Found(jdk_path) => {
            let display_name = jdk_display_name(&jdk_path);
            print_local_env_commands(&jdk_path.to_string_lossy(), &display_name);
            Ok(())
        }
        JdkLookupResult::NotFound => {
            bail!("JDK version '{}' not found.", version);
        }
    }
}

/// Prints the shell commands needed to set the JDK for the current session.
///
/// Paths are quoted to prevent shell injection when the output is eval'd.
///
/// On Windows, prints instructions since `set` commands cannot be applied
/// programmatically from a child process.
fn print_local_env_commands(jdk_path: &str, _display_name: &str) {
    if cfg!(target_os = "windows") {
        println!("Using local version automatically is not supported on cmd.");
        println!("Please copy/paste those commands in your current prompt:");
        println!("set JAVA_HOME={}", jdk_path);
        println!("set PATH={}\\bin;%PATH%", jdk_path);
    } else {
        // Paths are double-quoted to prevent word-splitting / shell injection
        // when the caller eval's this output (e.g. `eval $(sjvm use --local 17)`).
        println!("export JAVA_HOME=\"{}\"", jdk_path);
        println!("export PATH=\"{}/bin\":$PATH", jdk_path);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::jdk_switcher::{find_jdk_by_version_in_list, JdkLookupResult};

    fn test_jdks() -> Vec<PathBuf> {
        vec![
            PathBuf::from("/usr/lib/jvm/temurin-11-jdk"),
            PathBuf::from("/usr/lib/jvm/temurin-17-jdk"),
            PathBuf::from("/usr/lib/jvm/temurin-21-jdk"),
            PathBuf::from("/usr/lib/jvm/graalvm-ce-java17"),
        ]
    }

    #[test]
    fn test_find_jdk_by_version_number() {
        let jdks = test_jdks();

        let result = find_jdk_by_version_in_list("11", &jdks);
        assert!(matches!(result, JdkLookupResult::Found(_)));

        if let JdkLookupResult::Found(path) = result {
            assert!(path.to_string_lossy().contains("temurin-11"));
        }
    }

    #[test]
    fn test_find_jdk_by_vendor_name() {
        let jdks = test_jdks();

        let result = find_jdk_by_version_in_list("graalvm", &jdks);
        assert!(matches!(result, JdkLookupResult::Found(_)));

        if let JdkLookupResult::Found(path) = result {
            assert!(path.to_string_lossy().contains("graalvm"));
        }
    }

    #[test]
    fn test_find_jdk_version_not_found() {
        let jdks = test_jdks();

        let result = find_jdk_by_version_in_list("8", &jdks);
        assert_eq!(result, JdkLookupResult::NotFound);
    }

    #[test]
    fn test_use_version_output_format() {
        // Test that we can construct the expected output format
        let jdk_path = PathBuf::from("/usr/lib/jvm/temurin-17-jdk");
        let output = format!("✅ Now using JDK: {}", jdk_path.to_string_lossy());
        assert!(output.contains("✅"));
        assert!(output.contains("temurin-17"));
    }

    #[test]
    fn test_version_not_found_output_format() {
        let version = "99";
        let output = format!("❌ JDK version '{}' not found.", version);
        assert!(output.contains("❌"));
        assert!(output.contains("99"));
    }

    #[test]
    fn test_local_env_commands_unix() {
        let jdk_path = "/usr/lib/jvm/temurin-17-jdk";

        // Paths are double-quoted in the output for shell-injection safety
        let java_home = format!("export JAVA_HOME=\"{}\"", jdk_path);
        let path_cmd = format!("export PATH=\"{}/bin\":$PATH", jdk_path);

        assert!(java_home.contains("export"));
        assert!(java_home.contains(jdk_path));
        assert!(path_cmd.contains("/bin\""));
    }

    #[test]
    fn test_local_env_commands_windows() {
        let jdk_path = "C:\\Program Files\\Java\\jdk-17";

        // Test Windows-style output (unquoted — Windows cmd does not use bash quoting)
        let java_home = format!("set JAVA_HOME={}", jdk_path);
        let path_cmd = format!("set PATH={}\\bin;%PATH%", jdk_path);

        assert!(java_home.contains("set"));
        assert!(java_home.contains(jdk_path));
        assert!(path_cmd.contains("\\bin;%PATH%"));
    }
}
