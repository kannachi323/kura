use std::{env, path::PathBuf};

pub fn kura_data_root() -> Option<PathBuf> {
    resolve_desktop_home_dir()
}

pub fn kura_home_dir() -> Option<PathBuf> {
    kura_data_root().map(|root| root.join(".kura"))
}

fn resolve_desktop_home_dir() -> Option<PathBuf> {
    env::var_os("KURA_DATA_ROOT")
        .or_else(|| env::var_os("HOME"))
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}
