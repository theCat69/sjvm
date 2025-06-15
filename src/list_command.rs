use anyhow::Context;

use crate::memory::memory;
use crate::symlinks::get_symlink_path;

pub fn list_versions() {
    let current_link = get_symlink_path();
    let current = std::fs::read_link(&current_link)
        .with_context(|| "Cannot read current link")
        .unwrap();

    for jdk in &memory().jdks {
        let is_current = jdk == &current;
        let marker = if is_current { "→" } else { " " };
        println!("{} {}", marker, jdk.display());
    }
}
