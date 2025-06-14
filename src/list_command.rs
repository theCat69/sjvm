use crate::jdk_resolver::detect_jdks;
use crate::symlinks::get_symlink_path;

pub fn list_versions() {
    let current_link = get_symlink_path();
    let current = std::fs::read_link(&current_link).ok();

    for jdk in detect_jdks() {
        let is_current = Some(&jdk) == current.as_ref();
        let marker = if is_current { "→" } else { " " };
        println!("{} {}", marker, jdk.display());
    }
}
