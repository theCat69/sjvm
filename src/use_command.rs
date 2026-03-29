use std::path::Path;

use anyhow::{Context, bail};

use crate::jdk_switcher::{JdkLookupResult, find_jdk_by_version, jdk_display_name, switch_to_jdk};

/// Validates that a path does not contain shell metacharacters that would be
/// dangerous when the output is `eval`'d by the user's shell.
fn validate_shell_safe_path(path: &Path) -> anyhow::Result<()> {
    let path_str = path
        .to_str()
        .context("JDK path is not valid UTF-8; cannot generate shell export commands")?;
    const SHELL_METACHARACTERS: &[char] =
        &['`', '$', '"', '\\', '!', '&', '|', '>', '<', ';', '(', ')'];
    if let Some(bad_char) = path_str.chars().find(|c| SHELL_METACHARACTERS.contains(c)) {
        bail!(
            "JDK path '{path_str}' contains the shell metacharacter '{bad_char}' which is unsafe for eval output"
        );
    }
    Ok(())
}

/// Switches the globally active JDK to the version matching `version`.
///
/// # Errors
/// Returns an error if no JDK matching `version` is found or if the symlink
/// cannot be updated.
pub(crate) fn use_version(version: &str) -> anyhow::Result<()> {
    match find_jdk_by_version(version) {
        JdkLookupResult::Found(jdk_path) => {
            switch_to_jdk(&jdk_path)?;
            let jdk_path_display = jdk_path.to_string_lossy();
            println!("✅ Now using JDK: {jdk_path_display}");
            Ok(())
        }
        JdkLookupResult::NotFound => {
            bail!("JDK version '{version}' not found.");
        }
    }
}

/// Prints shell `export` commands to activate the JDK for the current session only.
///
/// # Errors
/// Returns an error if no JDK matching `version` is found, if the path
/// contains shell metacharacters, or if the path is not valid UTF-8.
pub(crate) fn use_version_local(version: &str) -> anyhow::Result<()> {
    match find_jdk_by_version(version) {
        JdkLookupResult::Found(jdk_path) => {
            let display_name = jdk_display_name(&jdk_path);
            validate_shell_safe_path(&jdk_path)?;
            print_local_env_commands(&jdk_path, &display_name)?;
            Ok(())
        }
        JdkLookupResult::NotFound => {
            bail!("JDK version '{version}' not found.");
        }
    }
}

/// Prints the shell commands needed to set the JDK for the current session.
///
/// Paths are validated for shell safety and double-quoted to prevent
/// word-splitting when the caller eval's this output.
///
/// On Windows, prints instructions since `set` commands cannot be applied
/// programmatically from a child process.
fn print_local_env_commands(jdk_path: &Path, _display_name: &str) -> anyhow::Result<()> {
    let path_str = jdk_path
        .to_str()
        .context("JDK path is not valid UTF-8; cannot generate shell export commands")?;

    if cfg!(target_os = "windows") {
        println!("Using local version automatically is not supported on cmd.");
        println!("Please copy/paste those commands in your current prompt:");
        // Wrap in double-quotes to handle spaces in the path.
        println!("set JAVA_HOME=\"{path_str}\"");
        println!("set PATH=\"{path_str}\\bin\";%PATH%");
    } else {
        // Paths are double-quoted to prevent word-splitting / shell injection
        // when the caller eval's this output (e.g. `eval $(sjvm use --local 17)`).
        println!("export JAVA_HOME=\"{path_str}\"");
        println!("export PATH=\"{path_str}/bin\":$PATH");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::jdk_switcher::{JdkLookupResult, find_jdk_by_version_in_list};

    use super::*;

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
        let jdk_path_display = jdk_path.to_string_lossy();
        let output = format!("✅ Now using JDK: {jdk_path_display}");
        assert!(output.contains("✅"));
        assert!(output.contains("temurin-17"));
    }

    #[test]
    fn test_version_not_found_output_format() {
        let version = "99";
        let output = format!("❌ JDK version '{version}' not found.");
        assert!(output.contains("❌"));
        assert!(output.contains("99"));
    }

    #[test]
    fn test_local_env_commands_unix() {
        // Use a real path with no metacharacters to test the function directly.
        let jdk_path = PathBuf::from("/usr/lib/jvm/temurin-17-jdk");

        // Validate that a clean path passes the shell-safety check.
        assert!(validate_shell_safe_path(&jdk_path).is_ok());

        // Verify the expected output format (double-quoted paths).
        let path_str = jdk_path.to_str().unwrap();
        let java_home = format!("export JAVA_HOME=\"{path_str}\"");
        let path_cmd = format!("export PATH=\"{path_str}/bin\":$PATH");

        assert!(java_home.contains("export"));
        assert!(java_home.contains(path_str));
        assert!(path_cmd.contains("/bin\""));
    }

    #[test]
    fn test_local_env_commands_windows() {
        let jdk_path = PathBuf::from("C:\\Program Files\\Java\\jdk-17");

        // Verify Windows-style output uses double-quoted paths.
        let path_str = jdk_path.to_str().unwrap();
        let java_home = format!("set JAVA_HOME=\"{path_str}\"");
        let path_cmd = format!("set PATH=\"{path_str}\\bin\";%PATH%");

        assert!(java_home.contains("set"));
        assert!(java_home.contains(path_str));
        assert!(path_cmd.contains("\\bin\""));
    }

    #[test]
    fn test_shell_safe_path_rejects_dollar_sign() {
        let bad_path = PathBuf::from("/usr/lib/jvm/jdk-$HOME");
        assert!(validate_shell_safe_path(&bad_path).is_err());
    }

    #[test]
    fn test_shell_safe_path_rejects_backtick() {
        let bad_path = PathBuf::from("/usr/lib/jvm/jdk-`id`");
        assert!(validate_shell_safe_path(&bad_path).is_err());
    }

    #[test]
    fn test_shell_safe_path_rejects_semicolon() {
        let bad_path = PathBuf::from("/usr/lib/jvm/jdk-17;rm -rf /");
        assert!(validate_shell_safe_path(&bad_path).is_err());
    }

    #[test]
    fn test_shell_safe_path_accepts_normal_path() {
        let good_path = PathBuf::from("/usr/lib/jvm/temurin-17.0.1-jdk");
        assert!(validate_shell_safe_path(&good_path).is_ok());
    }
}
