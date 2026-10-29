//! The engine the app talks to: remotes, browsing, working copies, uploads
//! and conflicts. Every cloud file has a local path under
//! `<providers_root>/<remote>/`, which exists once the file is downloaded.
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, oneshot, Mutex as AsyncMutex, OwnedMutexGuard};

use crate::{
    cloud::{
        auth::{PendingAuthorization, TokenSource},
        secrets::{RemoteSecret, SecretStore},
        AppCredentials, Provider, ProviderKind, Quota,
    },
    names,
    store::{RemoteRow, Store},
    Error, Result,
};

mod browse;
mod files;
mod ops;
#[cfg(test)]
mod tests;

pub struct KuragineConfig {
    /// `~/.kura/providers`: one folder per remote.
    pub providers_root: PathBuf,
    /// Where cloud versions of conflicting files, and local edits Kuragine
    /// moved aside, are kept.
    pub conflicts_dir: PathBuf,
    pub db_path: PathBuf,
    pub secrets: Arc<dyn SecretStore>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInfo {
    pub name: String,
    pub kind: ProviderKind,
    pub label: String,
    pub email: String,
    pub display_name: String,
    pub signed_in: bool,
    /// The remote's local folder.
    pub path: String,
}

/// A path inside a remote: its name and `/`-rooted remote path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CloudPath {
    pub remote: String,
    pub rel: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CopyState {
    /// Only in the cloud.
    Online,
    /// Downloaded and unchanged.
    Downloaded,
    /// Changed locally; an upload is due or running.
    Modified,
    /// The cloud copy changed too; waiting for a decision.
    Conflict,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudEntry {
    pub remote: String,
    pub rel: String,
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified_ms: Option<i64>,
    pub mime_type: Option<String>,
    /// Google Docs/Sheets/Slides exports.
    pub read_only: bool,
    pub state: CopyState,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictInfo {
    pub remote: String,
    pub rel: String,
    pub local_path: PathBuf,
    /// The cloud version, downloaded for comparison.
    pub remote_copy_path: PathBuf,
    pub detected_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictChoice {
    /// Overwrite the cloud copy with the local file.
    KeepMine,
    /// Replace the local file with the cloud version (the local one is kept aside).
    KeepTheirs,
    /// Upload the local file under a new name and take the cloud version.
    KeepBoth,
}

#[derive(Debug, Clone)]
pub enum UploadOutcome {
    Uploaded,
    /// The content matched the cloud already.
    Unchanged,
    /// Nothing to upload (missing file, folder, read-only export, scratch file).
    Skipped,
    Conflict(ConflictInfo),
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum EngineEvent {
    /// A folder's listing changed; `path` is its local path.
    #[serde(rename_all = "camelCase")]
    ListingChanged {
        remote: String,
        rel: String,
        path: String,
    },
    /// A downloaded file was replaced by a newer cloud version.
    #[serde(rename_all = "camelCase")]
    CopyUpdated {
        remote: String,
        rel: String,
        path: String,
    },
    /// A file changed on disk and should be uploaded.
    #[serde(rename_all = "camelCase")]
    UploadNeeded {
        remote: String,
        rel: String,
        path: String,
    },
    RemotesChanged,
    #[serde(rename_all = "camelCase")]
    SignInRequired {
        remote: String,
    },
}

#[derive(Clone)]
pub struct Kuragine {
    inner: Arc<Inner>,
}

struct Inner {
    root: PathBuf,
    staging: PathBuf,
    conflicts: PathBuf,
    store: Store,
    secrets: Arc<dyn SecretStore>,
    providers: Mutex<HashMap<String, Provider>>,
    needs_sign_in: Mutex<HashSet<String>>,
    events: broadcast::Sender<EngineEvent>,
    refreshing: Mutex<HashSet<CloudPath>>,
    locks: Mutex<HashMap<CloudPath, Arc<AsyncMutex<()>>>>,
    sign_ins: Mutex<HashMap<String, oneshot::Sender<()>>>,
}

pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

impl Kuragine {
    pub fn new(config: KuragineConfig) -> Result<Self> {
        let store = Store::open(&config.db_path)?;
        Ok(Self::with_store(config, store))
    }

    /// An engine whose index lives in memory, for when the database can't be
    /// opened. Cloud folders still work; they are listed again next launch.
    pub fn in_memory(config: KuragineConfig) -> Result<Self> {
        Ok(Self::with_store(config, Store::in_memory()?))
    }

    fn with_store(config: KuragineConfig, store: Store) -> Self {
        let (events, _) = broadcast::channel(512);
        Self {
            inner: Arc::new(Inner {
                staging: config.providers_root.join(".partial"),
                root: config.providers_root,
                conflicts: config.conflicts_dir,
                store,
                secrets: config.secrets,
                providers: Mutex::default(),
                needs_sign_in: Mutex::default(),
                events,
                refreshing: Mutex::default(),
                locks: Mutex::default(),
                sign_ins: Mutex::default(),
            }),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent> {
        self.inner.events.subscribe()
    }

    pub(crate) fn emit(&self, event: EngineEvent) {
        let _ = self.inner.events.send(event);
    }

    pub(crate) fn emit_listing(&self, remote: &str, rel: &str) {
        self.emit(EngineEvent::ListingChanged {
            remote: remote.to_owned(),
            rel: rel.to_owned(),
            path: self.local(remote, rel).to_string_lossy().into_owned(),
        });
    }

    pub(crate) fn store(&self) -> &Store {
        &self.inner.store
    }

    // Paths

    pub fn providers_root(&self) -> &Path {
        &self.inner.root
    }

    pub fn remote_root(&self, remote: &str) -> PathBuf {
        self.inner.root.join(remote)
    }

    /// The local path of a remote path.
    pub fn local(&self, remote: &str, rel: &str) -> PathBuf {
        names::local_path(&self.remote_root(remote), rel)
    }

    pub fn is_providers_root(&self, path: &Path) -> bool {
        path == self.inner.root
    }

    /// Maps a local path under the providers folder to its remote and remote
    /// path. Kuragine's own folders (`.partial`) and unknown remotes are `None`.
    pub fn resolve(&self, path: &Path) -> Option<CloudPath> {
        let relative = path.strip_prefix(&self.inner.root).ok()?;
        let mut components = relative.components();
        let first = components.next()?.as_os_str().to_str()?;
        if first.starts_with('.') {
            return None;
        }
        let remote = self.inner.store.remote(first).ok()??.name;
        let rel = names::rel_from_components(components.as_path()).ok()?;
        Some(CloudPath { remote, rel })
    }

    /// Holds a per-file lock so a download, upload or rename of one file
    /// never overlaps another.
    pub(crate) async fn lock(&self, remote: &str, rel: &str) -> OwnedMutexGuard<()> {
        let key = CloudPath {
            remote: remote.to_owned(),
            rel: rel.to_owned(),
        };
        let lock = {
            let mut locks = self.inner.locks.lock().unwrap();
            if locks.len() > 1024 {
                locks.retain(|_, lock| Arc::strong_count(lock) > 1);
            }
            locks.entry(key).or_default().clone()
        };
        lock.lock_owned().await
    }

    // Remotes

    pub fn remotes(&self) -> Result<Vec<RemoteInfo>> {
        Ok(self
            .inner
            .store
            .remotes()?
            .into_iter()
            .map(|row| self.remote_info(row))
            .collect())
    }

    pub fn remote(&self, name: &str) -> Result<RemoteInfo> {
        Ok(self.remote_info(self.remote_row(name)?))
    }

    fn remote_info(&self, row: RemoteRow) -> RemoteInfo {
        let signed_in =
            !row.email.is_empty() && !self.inner.needs_sign_in.lock().unwrap().contains(&row.name);
        RemoteInfo {
            path: self.remote_root(&row.name).to_string_lossy().into_owned(),
            label: row.kind.label().to_owned(),
            name: row.name,
            kind: row.kind,
            email: row.email,
            display_name: row.display_name,
            signed_in,
        }
    }

    pub(crate) fn remote_row(&self, name: &str) -> Result<RemoteRow> {
        self.inner
            .store
            .remote(name)?
            .ok_or_else(|| Error::NotFound(format!("The cloud account “{name}”")))
    }

    /// Registers a remote with the person's own OAuth app. It still needs
    /// `sign_in` before it can be used.
    pub async fn add_remote(
        &self,
        name: &str,
        kind: ProviderKind,
        credentials: AppCredentials,
    ) -> Result<RemoteInfo> {
        let name = name.trim();
        validate_remote_name(name)?;
        credentials.validate(kind)?;
        if self.inner.store.remote(name)?.is_some() {
            return Err(Error::invalid(format!(
                "A cloud account named “{name}” already exists."
            )));
        }
        self.save_secret(
            name,
            RemoteSecret {
                credentials,
                refresh_token: None,
            },
        )
        .await?;
        let row = RemoteRow {
            name: name.to_owned(),
            kind,
            email: String::new(),
            display_name: String::new(),
            drive_type: String::new(),
            created_ms: now_ms(),
        };
        self.inner.store.put_remote(&row)?;
        tokio::fs::create_dir_all(self.remote_root(name)).await?;
        self.emit(EngineEvent::RemotesChanged);
        Ok(self.remote_info(row))
    }

    /// Replaces a remote's app credentials. The old sign-in belonged to the old
    /// app, so the remote has to sign in again.
    pub async fn set_credentials(&self, name: &str, credentials: AppCredentials) -> Result<()> {
        let row = self.remote_row(name)?;
        credentials.validate(row.kind)?;
        self.save_secret(
            &row.name,
            RemoteSecret {
                credentials,
                refresh_token: None,
            },
        )
        .await?;
        self.inner.providers.lock().unwrap().remove(&row.name);
        self.inner.store.put_remote(&RemoteRow {
            email: String::new(),
            display_name: String::new(),
            ..row
        })?;
        self.emit(EngineEvent::RemotesChanged);
        Ok(())
    }

    /// Runs the browser sign-in. `open` receives the page to show.
    pub async fn sign_in<F>(&self, name: &str, open: F) -> Result<RemoteInfo>
    where
        F: FnOnce(&str) -> Result<()>,
    {
        let row = self.remote_row(name)?;
        let mut secret = self
            .load_secret(&row.name)
            .await?
            .ok_or_else(|| Error::invalid("Add this account's app credentials first."))?;
        let pending = PendingAuthorization::begin(row.kind, secret.credentials.clone()).await?;
        open(pending.url())?;
        let (cancel, canceled) = oneshot::channel();
        self.inner
            .sign_ins
            .lock()
            .unwrap()
            .insert(row.name.clone(), cancel);
        let result = tokio::select! {
            result = pending.finish() => result,
            _ = canceled => Err(Error::Canceled),
        };
        self.inner.sign_ins.lock().unwrap().remove(&row.name);
        let tokens = result?;

        secret.refresh_token = Some(tokens.refresh_token);
        self.save_secret(&row.name, secret.clone()).await?;
        let provider = Provider::new(
            row.kind,
            TokenSource::new(
                row.name.clone(),
                row.kind,
                secret,
                self.inner.secrets.clone(),
                Some((tokens.access_token, tokens.expires_in)),
            ),
        );
        let account = provider.account().await?;
        // A different account behind the same remote invalidates the index.
        if !row.email.is_empty() && !row.email.eq_ignore_ascii_case(&account.email) {
            self.inner.store.clear_items(&row.name)?;
        }
        let updated = RemoteRow {
            email: account.email,
            display_name: account.display_name,
            drive_type: account.drive_type,
            ..row
        };
        self.inner.store.put_remote(&updated)?;
        self.inner
            .providers
            .lock()
            .unwrap()
            .insert(updated.name.clone(), provider);
        self.inner
            .needs_sign_in
            .lock()
            .unwrap()
            .remove(&updated.name);
        self.emit(EngineEvent::RemotesChanged);
        Ok(self.remote_info(updated))
    }

    pub fn cancel_sign_in(&self, name: &str) {
        if let Some(cancel) = self.inner.sign_ins.lock().unwrap().remove(name) {
            let _ = cancel.send(());
        }
    }

    /// Forgets a remote and its sign-in. Its local folder is deleted only when
    /// `delete_files` is set.
    pub async fn remove_remote(&self, name: &str, delete_files: bool) -> Result<()> {
        let row = self.remote_row(name)?;
        self.cancel_sign_in(&row.name);
        let secrets = self.inner.secrets.clone();
        let remote = row.name.clone();
        tokio::task::spawn_blocking(move || secrets.delete(&remote))
            .await
            .map_err(|error| Error::Keychain(error.to_string()))??;
        self.inner.store.delete_remote(&row.name)?;
        self.inner.providers.lock().unwrap().remove(&row.name);
        if delete_files {
            match tokio::fs::remove_dir_all(self.remote_root(&row.name)).await {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        self.emit(EngineEvent::RemotesChanged);
        Ok(())
    }

    pub async fn quota(&self, name: &str) -> Result<Quota> {
        let row = self.remote_row(name)?;
        let provider = self.provider(&row.name).await?;
        self.noted(&row.name, provider.quota().await)
    }

    /// The signed-in provider for a remote.
    pub(crate) async fn provider(&self, remote: &str) -> Result<Provider> {
        if let Some(provider) = self.inner.providers.lock().unwrap().get(remote) {
            return Ok(provider.clone());
        }
        let row = self.remote_row(remote)?;
        let secret = self
            .load_secret(&row.name)
            .await?
            .filter(|secret| secret.refresh_token.is_some());
        let Some(secret) = secret else {
            return Err(self.note_auth(Error::AuthRequired(row.name.clone()), &row.name));
        };
        let provider = Provider::new(
            row.kind,
            TokenSource::new(
                row.name.clone(),
                row.kind,
                secret,
                self.inner.secrets.clone(),
                None,
            ),
        );
        self.inner
            .providers
            .lock()
            .unwrap()
            .insert(row.name.clone(), provider.clone());
        Ok(provider)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn set_provider(&self, remote: &str, provider: Provider) {
        self.inner
            .providers
            .lock()
            .unwrap()
            .insert(remote.to_owned(), provider);
    }

    /// Adds a signed-in Google Drive–shaped remote backed by an in-memory drive.
    #[cfg(any(test, feature = "test-support"))]
    pub fn connect_fake(&self, name: &str) -> Arc<crate::cloud::fake::FakeDrive> {
        self.inner
            .store
            .put_remote(&RemoteRow {
                name: name.to_owned(),
                kind: ProviderKind::GoogleDrive,
                email: "me@example.com".into(),
                display_name: "Me".into(),
                drive_type: String::new(),
                created_ms: now_ms(),
            })
            .expect("store the fake remote");
        let _ = std::fs::create_dir_all(self.remote_root(name));
        let drive = Arc::new(crate::cloud::fake::FakeDrive::new());
        self.set_provider(name, Provider::fake(drive.clone()));
        drive
    }

    /// Marks a remote as needing sign-in when `error` says so.
    pub(crate) fn note_auth(&self, error: Error, remote: &str) -> Error {
        if let Error::AuthRequired(_) = error {
            let first = self
                .inner
                .needs_sign_in
                .lock()
                .unwrap()
                .insert(remote.to_owned());
            if first {
                self.emit(EngineEvent::SignInRequired {
                    remote: remote.to_owned(),
                });
            }
            return Error::AuthRequired(remote.to_owned());
        }
        error
    }

    pub(crate) fn noted<T>(&self, remote: &str, result: Result<T>) -> Result<T> {
        result.map_err(|error| self.note_auth(error, remote))
    }

    async fn load_secret(&self, remote: &str) -> Result<Option<RemoteSecret>> {
        let secrets = self.inner.secrets.clone();
        let remote = remote.to_owned();
        tokio::task::spawn_blocking(move || secrets.load(&remote))
            .await
            .map_err(|error| Error::Keychain(error.to_string()))?
    }

    async fn save_secret(&self, remote: &str, secret: RemoteSecret) -> Result<()> {
        let secrets = self.inner.secrets.clone();
        let remote = remote.to_owned();
        tokio::task::spawn_blocking(move || secrets.save(&remote, &secret))
            .await
            .map_err(|error| Error::Keychain(error.to_string()))?
    }
}

/// Remote names become folder names, so they stay simple.
fn validate_remote_name(name: &str) -> Result<()> {
    if name.is_empty() || name.len() > 64 {
        return Err(Error::invalid("Name the account with 1 to 64 characters."));
    }
    if name.starts_with('.') || name.ends_with('.') || name.ends_with(' ') {
        return Err(Error::invalid(
            "An account name can't start with a dot or end with a dot or space.",
        ));
    }
    if !name
        .chars()
        .all(|ch| ch.is_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '.'))
    {
        return Err(Error::invalid(
            "Use letters, numbers, spaces, dots, dashes and underscores in the account name.",
        ));
    }
    Ok(())
}
