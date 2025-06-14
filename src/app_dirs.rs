use directories::ProjectDirs;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

static DIRS: OnceLock<AppDirs> = OnceLock::new();

pub struct AppDirs {
    pub data_dir: PathBuf,
    pub config_dir: PathBuf,
}

pub fn app_dirs() -> &'static AppDirs {
    DIRS.get_or_init(|| {
        let proj_dirs = init_proj_dir();
        AppDirs {
            data_dir: lazy_init_dirs(Some(proj_dirs.data_dir())),
            config_dir: lazy_init_dirs(Some(proj_dirs.config_dir())),
        }
    })
}

fn init_proj_dir() -> ProjectDirs {
    match ProjectDirs::from("rs", "", "sjvm") {
        Some(proj_dirs) => proj_dirs,
        None => panic!("Error creating config dir"),
    }
}

fn lazy_init_dirs(runtime_dir: Option<&std::path::Path>) -> PathBuf {
    match runtime_dir {
        Some(run_dir) => match fs::create_dir_all(run_dir) {
            Ok(()) => run_dir.to_path_buf(),
            Err(err) => panic!("Error creating config dir : {err}"),
        },
        //TODO what is fallback ?
        None => panic!("I should be able to create dir"),
    }
}
