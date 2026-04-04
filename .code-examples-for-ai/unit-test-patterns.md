<!-- Demonstrates: #[cfg(test)] mod tests pattern; use super::*; pure-function fixtures; error-message assertions -->

```rust
// ---------------------------------------------------------------------------
// Standard unit test module structure for sjvm
//
// - Placed at the bottom of the source file it tests
// - Gated with #[cfg(test)] so test code is never compiled into the binary
// - Uses `use super::*;` to access the parent module's private items
// ---------------------------------------------------------------------------

// Production code (example: commands/mod.rs)
pub(crate) fn validate_version_string(s: &str) -> Result<(), String> {
    if s.is_empty() {
        return Err("version cannot be empty".to_owned());
    }
    if s.len() > 64 {
        return Err("version string too long (max 64 chars)".to_owned());
    }
    if !s.chars().all(|c| c.is_alphanumeric() || "-._".contains(c)) {
        return Err(
            "version contains illegal characters (only alphanumeric, '-', '.', '_' allowed)"
                .to_owned(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    // Import all items from the parent module — gives access to private functions too
    use super::*;

    // 1. Simple assert on Ok/Err
    #[test]
    fn test_validate_version_string_rejects_empty() {
        assert!(validate_version_string("").is_err());
    }

    // 2. Parametric test using a slice of inputs — avoids repetition
    #[test]
    fn test_validate_version_string_rejects_metacharacters() {
        for bad in &["17;rm", "17$HOME", "17`id`", "17|cat", "17>out", "17("] {
            assert!(
                validate_version_string(bad).is_err(),
                "expected error for '{bad}'"  // message shown on failure
            );
        }
    }

    // 3. Test error message content, not just Ok/Err
    #[test]
    fn test_error_message_is_descriptive() {
        let err = validate_version_string("").unwrap_err();
        assert!(
            err.contains("empty"),
            "expected 'empty' in error, got: {err}"
        );
    }

    // 4. Test the happy path — critical to verify valid inputs ARE accepted
    #[test]
    fn test_validate_version_string_accepts_valid() {
        for good in &["17", "temurin-21", "graalvm-ce-java17", "1.8.0_391"] {
            assert!(
                validate_version_string(good).is_ok(),
                "expected ok for '{good}'"
            );
        }
    }

    // 5. Use anyhow::Result<()> as return type to use ? in tests
    #[test]
    fn test_with_result_return() -> anyhow::Result<()> {
        // Lets you use ? without .unwrap() for cleaner test code
        let result = some_fallible_function()?;
        assert_eq!(result, expected_value);
        Ok(())
    }

    // 6. Pure-function fixture pattern — inject data explicitly, avoid global state
    fn test_jdks() -> Vec<std::path::PathBuf> {
        vec![
            std::path::PathBuf::from("/usr/lib/jvm/temurin-11-jdk"),
            std::path::PathBuf::from("/usr/lib/jvm/temurin-17-jdk"),
            std::path::PathBuf::from("/usr/lib/jvm/graalvm-ce-java17"),
        ]
    }

    #[test]
    fn test_find_jdk_by_version_number() {
        let jdks = test_jdks();
        // Call the pure *_in_list variant — never the global-state-reading version
        let result = find_jdk_by_version_in_list("11", &jdks, None);
        assert!(!result.is_empty());
        assert!(result[0].to_string_lossy().contains("temurin-11"));
    }
}
```
