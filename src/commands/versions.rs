//! `sjvm versions` command — lists available JDK versions from vendor APIs.

use anyhow::Result;

use crate::core::jdk_catalog::{Vendor, fetch_available_versions};

/// Prints available JDK versions for the given vendor (or both if `None`).
///
/// Fetches data from the vendor API; requires a network connection.
pub(crate) fn run_versions(vendor: Option<&Vendor>) -> Result<()> {
    match vendor {
        Some(v) => print_vendor_versions(v)?,
        None => {
            print_vendor_versions(&Vendor::OpenJdk)?;
            print_vendor_versions(&Vendor::GraalVm)?;
        }
    }
    Ok(())
}

fn print_vendor_versions(vendor: &Vendor) -> Result<()> {
    let label = match vendor {
        Vendor::OpenJdk => "OpenJDK (Adoptium)",
        Vendor::GraalVm => "GraalVM CE",
    };
    let versions = fetch_available_versions(vendor)?;
    if versions.is_empty() {
        println!("{label}:");
        println!("  (no versions found)");
    } else {
        let version_list: Vec<String> = versions.iter().map(|v| v.to_string()).collect();
        println!("{label}: {}", version_list.join(", "));
    }
    Ok(())
}
