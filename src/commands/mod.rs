pub(crate) mod delete;
pub(crate) mod install;
pub(crate) mod list;
pub(crate) mod setup;
pub(crate) mod tag;
#[cfg(feature = "ui")]
pub(crate) mod ui;
pub(crate) mod use_cmd;
pub(crate) mod versions;

/// Validates a version string for use with the `use` and `install` commands.
///
/// Shared rules: not empty, max 64 characters, only alphanumeric or `-`, `.`, `_`.
pub(crate) fn validate_version_string(s: &str) -> Result<(), String> {
    if s.is_empty() {
        return Err("version cannot be empty".to_owned());
    }
    if s.len() > 64 {
        return Err("version string too long (max 64 chars)".to_owned());
    }
    if !s
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, '-' | '.' | '_'))
    {
        return Err(
            "version contains illegal characters (only alphanumeric, '-', '.', '_' allowed)"
                .to_owned(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_version_string;

    #[test]
    fn test_validate_version_string_rejects_empty() {
        assert!(validate_version_string("").is_err());
    }

    #[test]
    fn test_validate_version_string_rejects_too_long() {
        let long = "a".repeat(65);
        assert!(validate_version_string(&long).is_err());
    }

    #[test]
    fn test_validate_version_string_rejects_metacharacters() {
        for bad in &["17;rm", "17$HOME", "17`id`", "17|cat", "17>out", "17("] {
            assert!(
                validate_version_string(bad).is_err(),
                "expected error for '{bad}'"
            );
        }
    }

    #[test]
    fn test_validate_version_string_accepts_valid() {
        for good in &["17", "temurin-21", "graalvm-ce-java17", "1.8.0_391"] {
            assert!(
                validate_version_string(good).is_ok(),
                "expected ok for '{good}'"
            );
        }
    }

    #[test]
    fn test_validate_version_string_accepts_max_length() {
        let exactly_64 = "a".repeat(64);
        assert!(validate_version_string(&exactly_64).is_ok());
    }

    #[test]
    fn test_validate_version_string_rejects_space() {
        assert!(validate_version_string("17 beta").is_err());
        assert!(validate_version_string(" 17").is_err());
    }
}
