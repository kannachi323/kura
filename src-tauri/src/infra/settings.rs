//! Kura's settings: one JSON document in the app's config folder. Nothing here
//! syncs anywhere, so Open With associations are trusted as the person set them.
use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::{ApiError, ApiResult};
use crate::infra::environment::AppEnvironmentService;

#[derive(Debug, Clone)]
pub struct SettingsService {
    path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSnapshot {
    pub path: String,
    pub document: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenWithAssociation {
    pub key: String,
    pub application_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSettingsRequest {
    pub document: Value,
}

impl SettingsService {
    pub fn new(environment: AppEnvironmentService) -> Self {
        Self {
            path: environment.settings_path(),
        }
    }

    pub async fn snapshot(&self) -> ApiResult<SettingsSnapshot> {
        let path = self.path.clone();
        run(move || {
            let document = load_document(&path)?;
            Ok(snapshot(&path, document))
        })
        .await
    }

    pub async fn save(&self, request: SaveSettingsRequest) -> ApiResult<SettingsSnapshot> {
        let path = self.path.clone();
        run(move || {
            let document = match request.document {
                Value::Object(map) => Value::Object(map),
                _ => return Err(ApiError::Message("Settings must be an object.".into())),
            };
            save_document(&path, &document)?;
            Ok(snapshot(&path, document))
        })
        .await
    }

    pub async fn open_with_association_for_path(
        &self,
        file_path: String,
    ) -> ApiResult<Option<String>> {
        let path = self.path.clone();
        run(move || {
            let document = load_document(&path)?;
            let key = association_key_for_path(&file_path);
            Ok(document
                .get("open_with")
                .and_then(|open_with| open_with.get(&key))
                .and_then(Value::as_str)
                .filter(|application| {
                    !application.trim().is_empty() && Path::new(application).exists()
                })
                .map(str::to_owned))
        })
        .await
    }

    pub async fn open_with_associations(&self) -> ApiResult<Vec<OpenWithAssociation>> {
        let path = self.path.clone();
        run(move || {
            let document = load_document(&path)?;
            let mut associations: Vec<OpenWithAssociation> = document
                .get("open_with")
                .and_then(Value::as_object)
                .map(|open_with| {
                    open_with
                        .iter()
                        .filter_map(|(key, value)| {
                            Some(OpenWithAssociation {
                                key: key.clone(),
                                application_path: value.as_str()?.to_owned(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            associations.sort_by(|left, right| left.key.cmp(&right.key));
            Ok(associations)
        })
        .await
    }
}

async fn run<T: Send + 'static>(
    work: impl FnOnce() -> ApiResult<T> + Send + 'static,
) -> ApiResult<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|err| ApiError::Message(format!("Settings worker failed: {err}")))?
}

fn snapshot(path: &Path, document: Value) -> SettingsSnapshot {
    SettingsSnapshot {
        path: path.to_string_lossy().into_owned(),
        document,
    }
}

fn load_document(path: &Path) -> ApiResult<Value> {
    match fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(map)) => Ok(Value::Object(map)),
            // A damaged file falls back to defaults; the next save replaces it.
            _ => Ok(Value::Object(Map::new())),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(Value::Object(Map::new()))
        }
        Err(error) => Err(ApiError::Message(format!(
            "Could not read settings at {}: {error}",
            path.display()
        ))),
    }
}

fn save_document(path: &Path, document: &Value) -> ApiResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(document)?)?;
    fs::rename(&temporary, path)?;
    Ok(())
}

pub(crate) fn association_key_for_path(file_path: &str) -> String {
    let normalized = file_path.replace('\\', "/");
    let file_name = normalized
        .split('/')
        .filter(|part| !part.is_empty())
        .next_back()
        .unwrap_or(file_path);
    let key = file_name
        .rfind('.')
        .map(|index| &file_name[index..])
        .unwrap_or(file_name);
    key.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn association_keys_use_the_lowercase_extension() {
        assert_eq!(association_key_for_path("/a/b/Report.PDF"), ".pdf");
        assert_eq!(association_key_for_path("C:\\x\\notes.txt"), ".txt");
        assert_eq!(association_key_for_path("/a/Makefile"), "makefile");
    }

    #[test]
    fn settings_round_trip_through_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        assert_eq!(load_document(&path).unwrap(), Value::Object(Map::new()));
        let document = serde_json::json!({ "files": { "show_hidden_files": true } });
        save_document(&path, &document).unwrap();
        assert_eq!(load_document(&path).unwrap(), document);
    }
}
