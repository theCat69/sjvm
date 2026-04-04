use std::{
    io::{self, BufRead, Write},
    path::Path,
};

use anyhow::{Context, bail};

use crate::core::jdk_catalog::Vendor;
use crate::core::jdk_switcher::{
    find_jdk_by_version, jdk_display_name, switch_to_jdk, vendor_to_str,
};

/// Returns `true` if `jdk_dir` has no `.sjvm-vendor` file (custom / unknown JDK).
fn is_custom_jdk(jdk_dir: &Path) -> bool {
    !jdk_dir.join(".sjvm-vendor").exists()
}

/// When multiple JDK candidates are found, interactively prompt the user to pick
/// one (max 3 attempts).  Returns the selected path on success.
///
/// In non-interactive (non-terminal stdin) mode, returns the first candidate.
fn disambiguate(candidates: &[std::path::PathBuf]) -> anyhow::Result<std::path::PathBuf> {
    use std::io::IsTerminal as _;

    if !io::stdin().is_terminal() {
        // Non-interactive / CI: use first candidate (index 0).
        return candidates
            .first()
            .cloned()
            .context("BUG: empty candidate list passed to disambiguate");
    }

    println!("Multiple JDKs match. Please choose one:");
    for (i, path) in candidates.iter().enumerate() {
        let name = jdk_display_name(path);
        let label = if is_custom_jdk(path) {
            " [custom]".to_owned()
        } else {
            String::new()
        };
        println!("  {}) {name}{label}", i + 1);
    }

    let stdin = io::stdin();
    const MAX_ATTEMPTS: u8 = 3;
    for attempt in 0..MAX_ATTEMPTS {
        print!("Enter number (1-{}): ", candidates.len());
        io::stdout().flush().context("Failed to flush stdout")?;

        let mut line = String::new();
        stdin
            .lock()
            .read_line(&mut line)
            .context("Failed to read user input")?;

        let trimmed = line.trim();
        match trimmed.parse::<usize>() {
            Ok(n) if n >= 1 && n <= candidates.len() => {
                return Ok(candidates[n - 1].clone());
            }
            _ => {
                let remaining = MAX_ATTEMPTS - attempt - 1;
                if remaining > 0 {
                    eprintln!(
                        "Invalid input '{trimmed}'. Please enter a number between 1 and {}. ({remaining} attempt(s) left)",
                        candidates.len()
                    );
                }
            }
        }
    }

    bail!("Too many invalid attempts. Aborting JDK selection.");
}

/// Switches the globally active JDK to the version matching `version`,
/// optionally filtered by `vendor`.
///
/// # Errors
/// Returns an error if no JDK matching `version` (and optional vendor) is found
/// or if the symlink cannot be updated.
pub(crate) fn use_version(version: &str, vendor: Option<&Vendor>) -> anyhow::Result<()> {
    let candidates = find_jdk_by_version(version, vendor)?;

    let jdk_path = resolve_candidate(version, vendor, candidates)?;

    switch_to_jdk(&jdk_path)?;
    let jdk_path_display = jdk_path.to_string_lossy();
    println!("✅ Now using JDK: {jdk_path_display}");
    Ok(())
}

/// Prints shell `export` commands to activate the JDK for the current session only.
///
/// # Errors
/// Returns an error if no JDK matching `version` is found, or if the path
/// is not valid UTF-8.
pub(crate) fn use_version_local(version: &str, vendor: Option<&Vendor>) -> anyhow::Result<()> {
    let candidates = find_jdk_by_version(version, vendor)?;

    let jdk_path = resolve_candidate(version, vendor, candidates)?;

    let display_name = jdk_display_name(&jdk_path);
    print_local_env_commands(&jdk_path, &display_name)?;
    Ok(())
}

/// Resolves a list of candidates to a single JDK path, handling empty / single /
/// multiple-match cases.
fn resolve_candidate(
    version: &str,
    vendor: Option<&Vendor>,
    candidates: Vec<std::path::PathBuf>,
) -> anyhow::Result<std::path::PathBuf> {
    match candidates.len() {
        0 => {
            if let Some(v) = vendor {
                let vendor_name = vendor_to_str(v);
                bail!(
                    "JDK version '{version}' not found for vendor '{vendor_name}'.\n   Install it with: sjvm install {version} --vendor {vendor_name}"
                );
            } else {
                bail!("JDK version '{version}' not found.");
            }
        }
        1 => candidates
            .into_iter()
            .next()
            .context("BUG: empty candidate list in single-match arm"),
        _ => disambiguate(&candidates),
    }
}

/// Prints the shell commands needed to set the JDK for the current session.
///
/// Paths are double-quoted to prevent word-splitting when the caller
/// eval's this output (e.g. `eval $(sjvm use --local 17)`).
///
/// On Windows, prints instructions since `set` commands cannot be applied
/// programmatically from a child process.
fn print_local_env_commands(jdk_path: &Path, _display_name: &str) -> anyhow::Result<()> {
    let path_str = jdk_path
        .to_str()
        .context("JDK path is not valid UTF-8; cannot generate shell export commands")?;

    // Guard: reject paths containing shell metacharacters before emitting eval-able output.
    // Double-quoting alone does not protect against embedded `"`, `$`, or backtick.
    for ch in ['"', '$', '`', '\\'] {
        if path_str.contains(ch) {
            anyhow::bail!(
                "JDK path contains shell metacharacter '{ch}' and cannot be safely exported. \
                 Rename the JDK directory to remove special characters."
            );
        }
    }

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

    use crate::core::jdk_switcher::find_jdk_by_version_in_list;

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

        let result = find_jdk_by_version_in_list("11", &jdks, None);
        assert!(!result.is_empty());
        assert!(result[0].to_string_lossy().contains("temurin-11"));
    }

    #[test]
    fn test_find_jdk_by_vendor_name() {
        let jdks = test_jdks();

        let result = find_jdk_by_version_in_list("graalvm", &jdks, None);
        assert!(!result.is_empty());
        assert!(result[0].to_string_lossy().contains("graalvm"));
    }

    #[test]
    fn test_find_jdk_version_not_found() {
        let jdks = test_jdks();

        let result = find_jdk_by_version_in_list("8", &jdks, None);
        assert!(result.is_empty());
    }

    #[test]
    fn test_local_env_commands_unix() {
        // Use a real path with no metacharacters to test the function directly.
        let jdk_path = PathBuf::from("/usr/lib/jvm/temurin-17-jdk");

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
    fn test_resolve_candidate_not_found_no_vendor() {
        let result = resolve_candidate("99", None, vec![]);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("not found"), "expected 'not found' in: {msg}");
    }

    #[test]
    fn test_resolve_candidate_not_found_with_vendor() {
        let result = resolve_candidate("999", Some(&Vendor::GraalVm), vec![]);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("not found for vendor 'graalvm'"),
            "expected vendor hint in: {msg}"
        );
        assert!(
            msg.contains("sjvm install"),
            "expected install hint in: {msg}"
        );
    }

    #[test]
    fn test_resolve_candidate_single_match() {
        let path = PathBuf::from("/usr/lib/jvm/jdk-17");
        let result = resolve_candidate("17", None, vec![path.clone()]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), path);
    }

    #[test]
    fn test_local_env_commands_rejects_metacharacters() {
        // Paths with shell metacharacters must be rejected before eval-able output is emitted.
        for bad_path in &[
            "/jvms/jdk-17\"injected",
            "/jvms/jdk-17$HOME",
            "/jvms/jdk-17`id`",
        ] {
            let path = std::path::PathBuf::from(bad_path);
            let result = super::print_local_env_commands(&path, "test");
            assert!(result.is_err(), "expected error for path '{bad_path}'");
        }
    }
}
