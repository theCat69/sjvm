use crate::{
    jdk_resolver::detect_jdks,
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
        println!("Initial symlink set to: {:?}", first);
    } else {
        println!("No JDKs found.");
    }

    println!("\n✅ Setup complete.");
    if cfg!(target_os = "windows") {
        println!("➡️  Add C:\\Java\\current\\bin to your PATH.");
        println!("➡️  Add C:\\Java\\curentas your JAVA_HOME.");
    } else {
        println!("➡️  Add $HOME/.java/current/bin to your PATH.");
        println!("➡️  Add $HOME/.java/currenbin as your JAVA_HOME.");
    }
}
