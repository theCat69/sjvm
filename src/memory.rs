use bincode::{Decode, Encode, config};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use crate::{app_dirs::app_dirs, jdk_resolver::detect_jdks, symlinks::get_symlink_path};

static MEMORY: OnceLock<Memory> = OnceLock::new();
static MEMORY_FILE: OnceLock<PathBuf> = OnceLock::new();

#[derive(Encode, Decode, PartialEq, Debug)]
pub struct Memory {
    pub current: PathBuf,
    pub jdks: Vec<PathBuf>,
}

pub fn memory() -> &'static Memory {
    MEMORY.get_or_init(|| lazy_init_memory())
}

pub fn memory_file() -> &'static PathBuf {
    MEMORY_FILE.get_or_init(|| Path::join(&app_dirs().data_dir, "sjvm-mem"))
}

fn lazy_init_memory() -> Memory {
    let memory_file = memory_file();
    if !memory_file.is_file() {
        let current = get_current();
        let jdks = detect_jdks();
        let memory = Memory {
            current: current.to_path_buf(),
            jdks: jdks.to_owned(),
        };
        dump_binaries(&memory);
        return memory;
    } else {
        let memory = load_from_binaries();
        return memory;
    }
}

fn dump_binaries(memory: &Memory) {
    let _ = fs::write(
        memory_file(),
        bincode::encode_to_vec(memory, config::standard()).unwrap(),
    );
}

fn load_from_binaries() -> Memory {
    let file = fs::read(memory_file()).unwrap();
    let (decoded, _): (Memory, usize) =
        bincode::decode_from_slice(&file, config::standard()).unwrap();
    decoded
}

fn get_current() -> &'static PathBuf {
    let current_link = get_symlink_path();
    let current = std::fs::read_link(&current_link).ok();
    for jdk in detect_jdks() {
        let is_current = Some(jdk) == current.as_ref();
        if is_current {
            return jdk;
        }
    }
    panic!("No current jdks ! Did you run setup first ?")
}
