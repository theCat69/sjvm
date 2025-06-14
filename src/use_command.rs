use crate::jdk_resolver::detect_jdks;
use crate::symlinks::{create_symlink, get_symlink_path};

pub fn use_version(version: &str) {
    let jdks = detect_jdks();
    for jdk in jdks {
        if jdk.file_name().unwrap().to_string_lossy().contains(version) {
            let symlink = get_symlink_path();
            create_symlink(&jdk, &symlink).unwrap();
            println!("✅ Now using JDK: {:?}", jdk);
            return;
        }
    }
    println!("❌ JDK version '{}' not found.", version);
}
