use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::Serialize;

use crate::infra::paths;

#[derive(Clone)]
pub struct AppEnvironmentService {
    inner: Arc<AppEnvironment>,
}

#[derive(Debug, Clone)]
pub struct AppEnvironment {
    pub home_dir: PathBuf,
    pub kura_dir: PathBuf,
    pub config_dir: PathBuf,
    pub db_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub tmp_dir: PathBuf,
    pub settings_path: PathBuf,
    pub kura_config_path: PathBuf,
    pub workspaces_path: PathBuf,
    pub commands_path: PathBuf,
    pub config_exists: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppEnvironmentSnapshot {
    pub home_dir: String,
    pub kura_dir: String,
    pub config_dir: String,
    pub db_dir: String,
    pub cache_dir: String,
    pub tmp_dir: String,
    pub settings_path: String,
    pub kura_config_path: String,
    pub workspaces_path: String,
    pub commands_path: String,
    /// Retired remote-mount prefix, relative to the home directory.
    pub mount_path: String,
    pub config_exists: bool,
}

impl AppEnvironmentService {
    pub fn new() -> Self {
        Self::new_with_data_root(None)
    }

    pub fn new_with_data_root(data_root: Option<PathBuf>) -> Self {
        Self {
            inner: Arc::new(AppEnvironment::load(data_root)),
        }
    }

    pub fn snapshot(&self) -> AppEnvironmentSnapshot {
        self.inner.snapshot()
    }

    pub fn kura_config_path(&self) -> PathBuf {
        self.inner.kura_config_path.clone()
    }

    pub fn kura_db_path(&self) -> PathBuf {
        self.inner.db_dir.join("data.db")
    }

    pub fn settings_path(&self) -> PathBuf {
        self.inner.settings_path.clone()
    }

    pub fn commands_path(&self) -> PathBuf {
        self.inner.commands_path.clone()
    }

    pub fn config_dir(&self) -> PathBuf {
        self.inner.config_dir.clone()
    }

    pub fn home_dir(&self) -> PathBuf {
        self.inner.home_dir.clone()
    }

    /// Legacy remote-mount prefix; paths under it are treated as remote, never indexed.
    pub fn mount_root(&self) -> PathBuf {
        self.inner.kura_dir.join("mnt")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.inner.cache_dir.clone()
    }

    pub fn workspaces_path(&self) -> PathBuf {
        self.inner.workspaces_path.clone()
    }

    #[cfg(test)]
    pub fn for_test_home(home_dir: PathBuf) -> Self {
        Self {
            inner: Arc::new(AppEnvironment::for_home(home_dir)),
        }
    }
}

impl AppEnvironment {
    fn load(data_root: Option<PathBuf>) -> Self {
        let home_dir = data_root.or_else(resolve_home_dir).unwrap_or_default();
        let kura_dir = home_dir.join(".kura");
        let config_dir = kura_dir.join("config");
        let db_dir = kura_dir.join("db");
        let cache_dir = kura_dir.join(".cache");
        let tmp_dir = kura_dir.join("tmp");
        let settings_path = config_dir.join("settings.json");
        let kura_config_path = config_dir.join("kura.json");
        let workspaces_path = config_dir.join("workspaces.json");
        let commands_path = config_dir.join("commands.msy");

        Self {
            home_dir,
            kura_dir,
            config_dir,
            db_dir,
            cache_dir,
            tmp_dir,
            settings_path,
            kura_config_path: kura_config_path.clone(),
            workspaces_path,
            commands_path,
            config_exists: kura_config_path.exists(),
        }
    }

    #[cfg(test)]
    fn for_home(home_dir: PathBuf) -> Self {
        let kura_dir = home_dir.join(".kura");
        let config_dir = kura_dir.join("config");
        let db_dir = kura_dir.join("db");
        let cache_dir = kura_dir.join(".cache");
        let tmp_dir = kura_dir.join("tmp");
        let settings_path = config_dir.join("settings.json");
        let kura_config_path = config_dir.join("kura.json");
        let workspaces_path = config_dir.join("workspaces.json");
        let commands_path = config_dir.join("commands.msy");

        Self {
            home_dir,
            kura_dir,
            config_dir,
            db_dir,
            cache_dir,
            tmp_dir,
            settings_path,
            kura_config_path,
            workspaces_path,
            commands_path,
            config_exists: false,
        }
    }

    fn snapshot(&self) -> AppEnvironmentSnapshot {
        AppEnvironmentSnapshot {
            home_dir: display_path(&self.home_dir),
            kura_dir: display_path(&self.kura_dir),
            config_dir: display_path(&self.config_dir),
            db_dir: display_path(&self.db_dir),
            cache_dir: display_path(&self.cache_dir),
            tmp_dir: display_path(&self.tmp_dir),
            settings_path: display_path(&self.settings_path),
            kura_config_path: display_path(&self.kura_config_path),
            workspaces_path: display_path(&self.workspaces_path),
            commands_path: display_path(&self.commands_path),
            mount_path: ".kura/mnt".to_owned(),
            config_exists: self.config_exists,
        }
    }
}

fn resolve_home_dir() -> Option<PathBuf> {
    paths::kura_data_root()
}

fn display_path(path: &Path) -> String {
    clean_display_path(path.display().to_string().as_str())
}

#[cfg(windows)]
fn clean_display_path(path: &str) -> String {
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = path.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        path.to_string()
    }
}

#[cfg(not(windows))]
fn clean_display_path(path: &str) -> String {
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn unique_test_home(label: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!("kura-env-{label}-{nanos}"))
    }

    #[test]
    fn application_database_uses_existing_shared_data_db() {
        let root = env::temp_dir().join("kura-env-shared-data-db");
        let service = AppEnvironmentService {
            inner: Arc::new(AppEnvironment {
                home_dir: root.clone(),
                kura_dir: root.join(".kura"),
                config_dir: root.join(".kura/config"),
                db_dir: root.join(".kura/db"),
                cache_dir: root.join(".kura/.cache"),
                tmp_dir: root.join(".kura/tmp"),
                settings_path: root.join(".kura/config/settings.json"),
                kura_config_path: root.join(".kura/config/kura.json"),
                workspaces_path: root.join(".kura/config/workspaces.json"),
                commands_path: root.join(".kura/config/commands.msy"),
                config_exists: false,
            }),
        };

        assert_eq!(service.kura_db_path(), root.join(".kura/db/data.db"));
        assert_eq!(service.kura_db_path(), root.join(".kura/db/data.db"));
    }
}
