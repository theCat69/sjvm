//! `sjvm tag` command — assigns a vendor label to an existing JDK directory.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use crate::core::jdk_catalog::Vendor;
use crate::core::jdk_switcher::vendor_to_str;
use crate::infra::memory::memory;

/// Tags an existing JDK directory with a vendor label by writing `.sjvm-vendor`.
///
/// # Errors
/// - No JDK with that exact directory name is found in the cache.
/// - The JDK is already tagged and `force` is `false`.
/// - The `.sjvm-vendor` file cannot be written.
pub(crate) fn run_tag(name: &str, vendor: &Vendor, force: bool) -> Result<()> {
    let vendor_name = vendor_to_str(vendor);

    // Find the JDK directory by exact directory-name match in the cached list.
    let jdk_path: PathBuf = memory()?
        .jdks
        .iter()
        .find(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy() == name)
                .unwrap_or(false)
        })
        .cloned()
        .with_context(|| {
            format!("JDK '{name}' not found. Run 'sjvm list' to see available JDKs.")
        })?;

    let vendor_file = jdk_path.join(".sjvm-vendor");

    // Check if already tagged.
    if vendor_file.exists() && !force {
        let existing = std::fs::read_to_string(&vendor_file)
            .map(|s| s.trim().to_owned())
            .unwrap_or_default();
        bail!("'{name}' is already tagged as '{existing}'. Use --force to overwrite.");
    }

    std::fs::write(&vendor_file, vendor_name).with_context(|| {
        format!(
            "Failed to write .sjvm-vendor to '{}'",
            vendor_file.display()
        )
    })?;

    println!("✅ Tagged '{name}' as '{vendor_name}'");
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::core::jdk_catalog::Vendor;
    use crate::core::jdk_switcher::vendor_to_str;

    #[test]
    fn test_tag_vendor_to_string_openjdk() {
        assert_eq!(vendor_to_str(&Vendor::OpenJdk), "openjdk");
    }

    #[test]
    fn test_tag_vendor_to_string_graalvm() {
        assert_eq!(vendor_to_str(&Vendor::GraalVm), "graalvm");
    }
}
