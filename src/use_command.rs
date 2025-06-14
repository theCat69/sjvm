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

pub fn use_version_local(version: &str) {
    let jdks = detect_jdks();
    for jdk in jdks {
        if jdk.file_name().unwrap().to_string_lossy().contains(version) {
            println!("Using local version automatically in not supported on cmd.");
            println!("Please copy/paste those commands in your current prompt :");
            println!("set JAVA_HOME={}", &jdk.to_string_lossy());
            println!("set PATH={}\\bin;%PATH%", jdk.to_string_lossy());
            return;
        }
    }
    println!("❌ JDK version '{}' not found.", version);
}
