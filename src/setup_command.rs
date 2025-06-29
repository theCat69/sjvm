use std::fs;

use anyhow::Context;

use crate::{
    jdk_resolver::detect_jdks,
    memory::{memory, memory_file},
    symlinks::{create_symlink, get_symlink_path},
};

pub fn setup() {
    let symlink = get_symlink_path();

    if let Some(parent) = symlink.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let jdks = detect_jdks();
    if let Some(first) = jdks.first() {
        create_symlink(first, &symlink).unwrap();
        println!("Initial symlink set to: {}", first.to_string_lossy());
    } else {
        println!("No JDKs found.");
    }

    //Initiliazing memory
    let memory_file = memory_file();
    if memory_file.is_file() {
        fs::remove_file(memory_file)
            .with_context(|| "Cannot remove memory file")
            .unwrap();
    }
    let _ = memory();

    println!("\n✅ Setup complete.");
    if cfg!(target_os = "windows") {
        println!("=> Add C:\\Java\\current\\bin to your PATH.");
        println!("=> Add C:\\Java\\current as your JAVA_HOME.");
    } else {
        println!("=> Add $HOME/.java/current/bin to your PATH.");
        println!("=> Add $HOME/.java/current as your JAVA_HOME.");
    }
}
