//! Kura's data folder, `~/.kura`:
//!
//! ```text
//! ~/.kura/
//!   config/  kura.json (this layout), settings.json, navigation.json, saved-searches.json
//!   db/      data.db (transfers, operation queue, folder sizes, sync pairs)
//!   .cache/  explorer_state.json, search/, thumbnails/, trash/
//! ```
use std::{
    fs, io,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::{json, Value};

use crate::infra::environment::AppEnvironmentService;

pub const LAYOUT_VERSION: u64 = 1;

/// Create the layout. Failures are reported to stderr and never stop Kura from starting.
pub fn prepare(environment: &AppEnvironmentService) {
    if let Err(error) = prepare_layout(environment) {
        eprintln!(
            "kura: could not prepare {}: {error}",
            environment.kura_dir().display()
        );
    }
}

fn prepare_layout(environment: &AppEnvironmentService) -> io::Result<()> {
    for dir in [
        environment.kura_dir(),
        environment.config_dir(),
        environment.db_dir(),
        environment.cache_dir(),
    ] {
        create_private_dir(&dir)?;
    }
    let marker = environment.kura_config_path();
    if marker.exists() {
        return Ok(());
    }
    let document = json!({
        "schema_version": LAYOUT_VERSION,
        "created_at_ms": now_ms(),
    });
    write_json(&marker, &document)
}

/// Write beside the target, then rename, so a crash never leaves half a file.
fn write_json(path: &Path, value: &Value) -> io::Result<()> {
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    fs::write(&temporary, bytes)?;
    fs::rename(&temporary, path)
}

fn create_private_dir(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_home(label: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("kura-data-home-{label}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn read_json(path: &Path) -> Value {
        serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
    }

    #[test]
    fn creates_the_layout_in_kura_home_only() {
        let home = temp_home("layout");
        let environment = AppEnvironmentService::for_test_home(home.clone());

        prepare(&environment);

        for dir in ["config", "db", ".cache"] {
            assert!(home.join(".kura").join(dir).is_dir(), "{dir} missing");
        }
        let marker = read_json(&environment.kura_config_path());
        assert_eq!(marker["schema_version"], json!(LAYOUT_VERSION));
        assert!(!environment.settings_path().exists());
    }

    #[test]
    fn keeps_an_existing_layout_marker() {
        let home = temp_home("again");
        let environment = AppEnvironmentService::for_test_home(home);
        prepare(&environment);
        let first = fs::read(environment.kura_config_path()).unwrap();
        prepare(&environment);
        assert_eq!(fs::read(environment.kura_config_path()).unwrap(), first);
    }
}
