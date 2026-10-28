//! Working copies: downloading on open, keeping unchanged copies fresh,
//! uploading local changes and stopping at conflicts.
use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use super::{
    browse::{local_state, make_writable, snapshot},
    now_ms, ConflictChoice, ConflictInfo, EngineEvent, Kuragine, UploadOutcome,
};
use crate::{
    cloud::{NoProgress, Provider, RemoteItem, TransferObserver, UploadTarget},
    hash::{hash_file, same_hash},
    names,
    store::{ConflictRow, CopyRow, ItemRow},
    Error, Result,
};

/// How long a freshness check counts for before opening checks again.
const FRESH_FOR_MS: i64 = 20_000;

/// Whether the cloud item's content differs from a copy's base.
pub(crate) fn content_differs(current: &RemoteItem, base_hash: &str, base_revision: &str) -> bool {
    match current.hash.as_deref() {
        Some(hash) if !base_hash.is_empty() => !same_hash(hash, base_hash),
        _ => current.revision.as_deref().unwrap_or_default() != base_revision,
    }
}

fn time_from_ms(ms: i64) -> Option<SystemTime> {
    let ms = u64::try_from(ms).ok()?;
    UNIX_EPOCH.checked_add(Duration::from_millis(ms))
}

/// Gives a downloaded file the cloud's modified time (and makes exports
/// read-only) before it moves into place.
async fn finish_download(path: &Path, item: &RemoteItem) -> Result<()> {
    if let Some(time) = item.modified_ms.and_then(time_from_ms) {
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            std::fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .and_then(|file| file.set_modified(time))
        })
        .await
        .map_err(|error| Error::Io(std::io::Error::other(error)))??;
    }
    if item.export_mime.is_some() {
        let mut permissions = tokio::fs::metadata(path).await?.permissions();
        permissions.set_readonly(true);
        tokio::fs::set_permissions(path, permissions).await?;
    }
    Ok(())
}

async fn move_file(from: &Path, to: &Path) -> Result<()> {
    if let Some(parent) = to.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    if tokio::fs::metadata(to).await.is_ok() {
        let _ = make_writable(to).await;
    }
    match tokio::fs::rename(from, to).await {
        Ok(()) => Ok(()),
        // Different volumes: copy, then remove.
        Err(_) => {
            tokio::fs::copy(from, to).await?;
            tokio::fs::remove_file(from).await?;
            Ok(())
        }
    }
}

fn conflict_name(name: &str) -> String {
    let stamp = chrono::Local::now().format("%Y-%m-%d %H%M");
    match names::split_extension(name) {
        (stem, Some(extension)) => format!("{stem} (conflict {stamp}).{extension}"),
        (stem, None) => format!("{stem} (conflict {stamp})"),
    }
}

impl Kuragine {
    /// Makes a cloud file available locally and returns its path. An existing
    /// copy is returned at once and checked against the cloud in the background.
    pub async fn ensure_local(
        &self,
        remote: &str,
        rel: &str,
        observer: &dyn TransferObserver,
    ) -> Result<PathBuf> {
        let remote = self.remote_row(remote)?.name;
        let result = self.ensure_local_inner(&remote, rel, observer).await;
        self.noted(&remote, result)
    }

    async fn ensure_local_inner(
        &self,
        remote: &str,
        rel: &str,
        observer: &dyn TransferObserver,
    ) -> Result<PathBuf> {
        let row = self.item_row(remote, rel).await?;
        let local = self.local(remote, rel);
        if row.item.is_dir {
            tokio::fs::create_dir_all(&local).await?;
            return Ok(local);
        }
        let _guard = self.lock(remote, rel).await;
        let copy = self.store().copy(remote, rel)?;
        let exists = tokio::fs::metadata(&local)
            .await
            .is_ok_and(|meta| meta.is_file());
        match copy {
            Some(copy) if exists => {
                if now_ms() - copy.checked_ms > FRESH_FOR_MS {
                    self.spawn_freshness_check(remote, rel);
                }
                Ok(local)
            }
            None if exists => {
                // A file Kura didn't download is here. Adopt it if it matches
                // the cloud; otherwise leave it for the upload to sort out.
                let provider = self.provider(remote).await?;
                let hash = hash_file(&local, provider.kind().hash_kind()).await?;
                if row
                    .item
                    .hash
                    .as_deref()
                    .is_some_and(|remote| same_hash(remote, &hash))
                {
                    self.record_copy(remote, &row, &hash, &local, now_ms())?;
                }
                Ok(local)
            }
            _ => {
                self.download_copy(remote, &row, observer).await?;
                Ok(local)
            }
        }
    }

    /// Downloads a file into its local path. The caller holds the file's lock.
    async fn download_copy(
        &self,
        remote: &str,
        row: &ItemRow,
        observer: &dyn TransferObserver,
    ) -> Result<()> {
        let provider = self.provider(remote).await?;
        tokio::fs::create_dir_all(&self.inner.staging).await?;
        let staging = self.inner.staging.join(uuid::Uuid::new_v4().to_string());
        let downloaded = match provider.download(&row.item, &staging, observer).await {
            Ok(downloaded) => downloaded,
            Err(error) => {
                let _ = tokio::fs::remove_file(&staging).await;
                return Err(error);
            }
        };
        if let Some(expected) = row.item.hash.as_deref() {
            if !same_hash(expected, &downloaded.hash) {
                let _ = tokio::fs::remove_file(&staging).await;
                return Err(Error::Network(format!(
                    "{} didn't match the cloud's checksum after downloading. Try again.",
                    row.item.name
                )));
            }
        }
        finish_download(&staging, &row.item).await?;
        let local = self.local(remote, &row.path);
        if tokio::fs::metadata(&local).await.is_ok() {
            let unchanged = self
                .store()
                .copy(remote, &row.path)?
                .is_some_and(|copy| local_state(&local, &copy) == Some(false));
            if !unchanged {
                self.set_aside(remote, &row.path).await?;
            }
        }
        if let Err(error) = move_file(&staging, &local).await {
            let _ = tokio::fs::remove_file(&staging).await;
            return Err(error);
        }
        self.record_copy(remote, row, &downloaded.hash, &local, now_ms())
    }

    fn record_copy(
        &self,
        remote: &str,
        row: &ItemRow,
        hash: &str,
        local: &Path,
        checked_ms: i64,
    ) -> Result<()> {
        let (local_size, local_mtime_ns) = snapshot(local)?;
        self.store().put_copy(
            remote,
            &CopyRow {
                path: row.path.clone(),
                id: row.item.id.clone(),
                base_hash: hash.to_owned(),
                base_revision: row.item.revision.clone().unwrap_or_default(),
                local_size,
                local_mtime_ns,
                downloaded_ms: now_ms(),
                read_only: row.item.export_mime.is_some(),
                checked_ms,
            },
        )
    }

    /// Moves a local file out of the way into the conflicts folder, so nothing
    /// Kuragine replaces is ever lost. Returns where it went.
    pub(crate) async fn set_aside(&self, remote: &str, rel: &str) -> Result<PathBuf> {
        let local = self.local(remote, rel);
        let stamp = chrono::Local::now().format("%Y-%m-%d %H%M%S");
        let destination = self
            .inner
            .conflicts
            .join(remote)
            .join("kept")
            .join(format!("{stamp} {}", names::file_name(rel)));
        move_file(&local, &destination).await?;
        Ok(destination)
    }

    pub(crate) fn spawn_freshness_check(&self, remote: &str, rel: &str) {
        let engine = self.clone();
        let remote = remote.to_owned();
        let rel = rel.to_owned();
        tokio::spawn(async move {
            if let Err(error) = engine.check_freshness(&remote, &rel).await {
                engine.note_auth(error, &remote);
            }
        });
    }

    /// Replaces an unchanged local copy when the cloud has a newer version.
    pub async fn check_freshness(&self, remote: &str, rel: &str) -> Result<()> {
        let _guard = self.lock(remote, rel).await;
        let (Some(mut copy), Some(row)) = (
            self.store().copy(remote, rel)?,
            self.store().item(remote, rel)?,
        ) else {
            return Ok(());
        };
        let provider = self.provider(remote).await?;
        let current = match provider.stat(&row.item).await {
            Ok(current) => current,
            Err(error) if error.is_not_found() => {
                let copies = self.store().delete_subtree(remote, rel)?;
                self.drop_unchanged_copies(remote, &copies).await;
                self.emit_listing(remote, &names::parent(rel));
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        let current_row = ItemRow {
            item: current,
            ..row
        };
        self.store().put_item(remote, &current_row)?;
        copy.checked_ms = now_ms();
        self.store().put_copy(remote, &copy)?;
        let local = self.local(remote, rel);
        let remote_changed = copy.id != current_row.item.id
            || content_differs(&current_row.item, &copy.base_hash, &copy.base_revision);
        if remote_changed && local_state(&local, &copy) == Some(false) {
            self.download_copy(remote, &current_row, &NoProgress)
                .await?;
            self.emit(EngineEvent::CopyUpdated {
                remote: remote.to_owned(),
                rel: rel.to_owned(),
                path: local.to_string_lossy().into_owned(),
            });
            self.emit_listing(remote, &names::parent(rel));
        }
        Ok(())
    }

    /// Uploads a local file if its content changed, checking the cloud first.
    pub async fn upload_copy(
        &self,
        remote: &str,
        rel: &str,
        observer: &dyn TransferObserver,
    ) -> Result<UploadOutcome> {
        let remote = self.remote_row(remote)?.name;
        let _guard = self.lock(&remote, rel).await;
        let result = self.upload_locked(&remote, rel, observer).await;
        self.noted(&remote, result)
    }

    async fn upload_locked(
        &self,
        remote: &str,
        rel: &str,
        observer: &dyn TransferObserver,
    ) -> Result<UploadOutcome> {
        if rel == names::ROOT || names::is_scratch_name(names::file_name(rel)) {
            return Ok(UploadOutcome::Skipped);
        }
        let local = self.local(remote, rel);
        // Snapshot before hashing: a write during the upload then still looks
        // like a change afterwards.
        let Ok((size, mtime)) = snapshot(&local) else {
            return Ok(UploadOutcome::Skipped);
        };
        if !local.is_file() {
            return Ok(UploadOutcome::Skipped);
        }
        let copy = self.store().copy(remote, rel)?;
        if copy.as_ref().is_some_and(|copy| copy.read_only) {
            return Ok(UploadOutcome::Skipped);
        }
        if let Some(conflict) = self.store().conflict(remote, rel)? {
            return Ok(UploadOutcome::Conflict(self.conflict_info(conflict)));
        }
        let provider = self.provider(remote).await?;
        let local_hash = hash_file(&local, provider.kind().hash_kind()).await?;
        if let Some(copy) = &copy {
            if same_hash(&local_hash, &copy.base_hash) {
                self.store().put_copy(
                    remote,
                    &CopyRow {
                        local_size: size,
                        local_mtime_ns: mtime,
                        ..copy.clone()
                    },
                )?;
                return Ok(UploadOutcome::Unchanged);
            }
        }

        let parent_rel = names::parent(rel);
        let parent = self.ensure_remote_folder(remote, &parent_rel).await?;
        let mut existing = self.store().item(remote, rel)?;
        if existing.is_none() {
            // Make sure no one else created this name since the last listing.
            self.refresh_children(remote, &parent).await?;
            existing = self.store().item(remote, rel)?;
        }
        let current = match &existing {
            Some(row) => match provider.stat(&row.item).await {
                Ok(item) => Some(item),
                Err(error) if error.is_not_found() => None,
                Err(error) => return Err(error),
            },
            None => None,
        };
        if let Some(current) = &current {
            if current.is_dir {
                return Err(Error::invalid(format!(
                    "The cloud has a folder named {}.",
                    names::file_name(rel)
                )));
            }
            let base_matches = match &copy {
                Some(copy) => {
                    copy.id == current.id
                        && !content_differs(current, &copy.base_hash, &copy.base_revision)
                }
                None => false,
            };
            if !base_matches {
                if current
                    .hash
                    .as_deref()
                    .is_some_and(|hash| same_hash(hash, &local_hash))
                {
                    // Both sides already hold the same content.
                    let row = ItemRow {
                        path: rel.to_owned(),
                        parent_path: parent_rel,
                        item: current.clone(),
                        listed_ms: 0,
                    };
                    self.store().put_item(remote, &row)?;
                    self.record_copy(remote, &row, &local_hash, &local, now_ms())?;
                    return Ok(UploadOutcome::Unchanged);
                }
                let conflict = self
                    .record_conflict(remote, rel, current, &provider)
                    .await?;
                return Ok(UploadOutcome::Conflict(conflict));
            }
        }

        let name = current
            .as_ref()
            .map(|item| item.name.clone())
            .unwrap_or_else(|| names::file_name(rel).to_owned());
        let target = UploadTarget {
            parent: &parent.item,
            name: &name,
            existing: current
                .as_ref()
                .map(|item| (item, item.revision.as_deref())),
        };
        let uploaded = match provider.upload(target, &local, observer).await {
            Ok(uploaded) => uploaded,
            Err(Error::Conflict(_)) => {
                // The cloud changed between the check and the upload.
                let latest = match &current {
                    Some(item) => provider.stat(item).await?,
                    None => {
                        self.refresh_children(remote, &parent).await?;
                        let row = self
                            .store()
                            .item(remote, rel)?
                            .ok_or_else(|| Error::Conflict(name.clone()))?;
                        row.item
                    }
                };
                let conflict = self
                    .record_conflict(remote, rel, &latest, &provider)
                    .await?;
                return Ok(UploadOutcome::Conflict(conflict));
            }
            Err(error) => return Err(error),
        };
        if let Some(hash) = uploaded.hash.as_deref() {
            if !same_hash(hash, &local_hash) {
                return Err(Error::Network(format!(
                    "{} changed while it was uploading. It will upload again.",
                    names::file_name(rel)
                )));
            }
        }
        let row = ItemRow {
            path: rel.to_owned(),
            parent_path: parent_rel.clone(),
            item: uploaded,
            listed_ms: 0,
        };
        self.store().put_item(remote, &row)?;
        self.store().put_copy(
            remote,
            &CopyRow {
                path: rel.to_owned(),
                id: row.item.id.clone(),
                base_hash: local_hash,
                base_revision: row.item.revision.clone().unwrap_or_default(),
                local_size: size,
                local_mtime_ns: mtime,
                downloaded_ms: now_ms(),
                read_only: false,
                checked_ms: now_ms(),
            },
        )?;
        self.emit_listing(remote, &parent_rel);
        Ok(UploadOutcome::Uploaded)
    }

    /// The cloud folder for a remote path, creating it (and its parents) in the
    /// cloud when it exists only locally.
    pub(crate) async fn ensure_remote_folder(&self, remote: &str, rel: &str) -> Result<ItemRow> {
        match self.item_row(remote, rel).await {
            Ok(row) if row.item.is_dir => return Ok(row),
            Ok(_) => {
                return Err(Error::invalid(format!(
                    "{} is a file in the cloud.",
                    names::file_name(rel)
                )))
            }
            Err(error) if error.is_not_found() => {}
            Err(error) => return Err(error),
        }
        let parent = Box::pin(self.ensure_remote_folder(remote, &names::parent(rel))).await?;
        let provider = self.provider(remote).await?;
        let created = provider
            .create_folder(&parent.item, names::file_name(rel))
            .await?;
        let row = ItemRow {
            path: rel.to_owned(),
            parent_path: parent.path.clone(),
            item: created,
            listed_ms: now_ms(),
        };
        self.store().put_item(remote, &row)?;
        self.emit_listing(remote, &parent.path);
        Ok(row)
    }

    async fn record_conflict(
        &self,
        remote: &str,
        rel: &str,
        current: &RemoteItem,
        provider: &Provider,
    ) -> Result<ConflictInfo> {
        let directory = self
            .inner
            .conflicts
            .join(remote)
            .join(uuid::Uuid::new_v4().to_string());
        tokio::fs::create_dir_all(&directory).await?;
        let theirs = directory.join(names::file_name(rel));
        provider.download(current, &theirs, &NoProgress).await?;
        finish_download(&theirs, current).await?;
        let row = ConflictRow {
            remote: remote.to_owned(),
            path: rel.to_owned(),
            id: current.id.clone(),
            remote_revision: current.revision.clone().unwrap_or_default(),
            remote_hash: current.hash.clone().unwrap_or_default(),
            remote_copy: theirs.to_string_lossy().into_owned(),
            detected_ms: now_ms(),
        };
        self.store().put_conflict(&row)?;
        self.emit_listing(remote, &names::parent(rel));
        Ok(self.conflict_info(row))
    }

    fn conflict_info(&self, row: ConflictRow) -> ConflictInfo {
        ConflictInfo {
            local_path: self.local(&row.remote, &row.path),
            remote_copy_path: PathBuf::from(&row.remote_copy),
            remote: row.remote,
            rel: row.path,
            detected_ms: row.detected_ms,
        }
    }

    pub fn conflicts(&self) -> Result<Vec<ConflictInfo>> {
        Ok(self
            .store()
            .conflicts()?
            .into_iter()
            .map(|row| self.conflict_info(row))
            .collect())
    }

    pub fn conflict(&self, remote: &str, rel: &str) -> Result<Option<ConflictInfo>> {
        Ok(self
            .store()
            .conflict(remote, rel)?
            .map(|row| self.conflict_info(row)))
    }

    /// Settles a conflict. Nothing is deleted: a local edit that loses is
    /// moved to the conflicts folder.
    pub async fn resolve_conflict(
        &self,
        remote: &str,
        rel: &str,
        choice: ConflictChoice,
        observer: &dyn TransferObserver,
    ) -> Result<UploadOutcome> {
        let remote = self.remote_row(remote)?.name;
        let guard = self.lock(&remote, rel).await;
        let result = self
            .resolve_locked(&remote, rel, choice, observer, guard)
            .await;
        self.noted(&remote, result)
    }

    async fn resolve_locked(
        &self,
        remote: &str,
        rel: &str,
        choice: ConflictChoice,
        observer: &dyn TransferObserver,
        guard: tokio::sync::OwnedMutexGuard<()>,
    ) -> Result<UploadOutcome> {
        let conflict = self
            .store()
            .conflict(remote, rel)?
            .ok_or_else(|| Error::invalid("This file has no conflict to resolve."))?;
        let theirs = PathBuf::from(&conflict.remote_copy);
        let provider = self.provider(remote).await?;
        let parent_rel = names::parent(rel);
        let outcome = match choice {
            ConflictChoice::KeepMine => {
                // Rebase the copy on the cloud version, then upload over it.
                let local = self.local(remote, rel);
                let theirs_hash = hash_file(&theirs, provider.kind().hash_kind()).await?;
                let (local_size, local_mtime_ns) = snapshot(&local)?;
                self.store().put_copy(
                    remote,
                    &CopyRow {
                        path: rel.to_owned(),
                        id: conflict.id.clone(),
                        base_hash: theirs_hash,
                        base_revision: conflict.remote_revision.clone(),
                        local_size,
                        local_mtime_ns,
                        downloaded_ms: now_ms(),
                        read_only: false,
                        checked_ms: now_ms(),
                    },
                )?;
                self.store().delete_conflict(remote, rel)?;
                self.upload_locked(remote, rel, observer).await?
            }
            ConflictChoice::KeepTheirs => {
                if tokio::fs::metadata(self.local(remote, rel)).await.is_ok() {
                    self.set_aside(remote, rel).await?;
                }
                self.take_theirs(remote, rel, &conflict, &theirs, &provider)
                    .await?;
                UploadOutcome::Unchanged
            }
            ConflictChoice::KeepBoth => {
                let local = self.local(remote, rel);
                let mine_rel = names::join(&parent_rel, &conflict_name(names::file_name(rel)));
                move_file(&local, &self.local(remote, &mine_rel)).await?;
                self.take_theirs(remote, rel, &conflict, &theirs, &provider)
                    .await?;
                drop(guard);
                let _mine_guard = self.lock(remote, &mine_rel).await;
                self.upload_locked(remote, &mine_rel, observer).await?
            }
        };
        if let Some(directory) = theirs.parent() {
            let _ = tokio::fs::remove_dir_all(directory).await;
        }
        // Keeping mine can run into a newer cloud change; that conflict stands.
        if !matches!(outcome, UploadOutcome::Conflict(_)) {
            self.store().delete_conflict(remote, rel)?;
        }
        self.emit_listing(remote, &parent_rel);
        Ok(outcome)
    }

    /// Moves the downloaded cloud version into place as the new base.
    async fn take_theirs(
        &self,
        remote: &str,
        rel: &str,
        conflict: &ConflictRow,
        theirs: &Path,
        provider: &Provider,
    ) -> Result<()> {
        let hash = hash_file(theirs, provider.kind().hash_kind()).await?;
        let local = self.local(remote, rel);
        move_file(theirs, &local).await?;
        let row = match self.store().item(remote, rel)? {
            Some(row) => row,
            None => ItemRow {
                path: rel.to_owned(),
                parent_path: names::parent(rel),
                item: RemoteItem {
                    id: conflict.id.clone(),
                    name: names::file_name(rel).to_owned(),
                    ..RemoteItem::default()
                },
                listed_ms: 0,
            },
        };
        let row = ItemRow {
            item: RemoteItem {
                id: conflict.id.clone(),
                revision: Some(conflict.remote_revision.clone()),
                hash: (!conflict.remote_hash.is_empty()).then(|| conflict.remote_hash.clone()),
                ..row.item
            },
            ..row
        };
        self.store().put_item(remote, &row)?;
        self.record_copy(remote, &row, &hash, &local, now_ms())
    }

    /// Called when a file under the providers folder settled after changing.
    /// Asks for an upload when its content differs from the copy's base.
    pub async fn local_change(&self, path: &Path) {
        let Some(cloud) = self.resolve(path) else {
            return;
        };
        if cloud.rel == names::ROOT {
            return;
        }
        if path.is_dir() {
            for file in files_under(path).await {
                Box::pin(self.local_change(&file)).await;
            }
            return;
        }
        if !path.is_file() || names::is_scratch_name(names::file_name(&cloud.rel)) {
            return;
        }
        if let Ok(Some(copy)) = self.store().copy(&cloud.remote, &cloud.rel) {
            if copy.read_only || local_state(path, &copy) != Some(true) {
                return;
            }
            let Ok(provider) = self.provider(&cloud.remote).await else {
                return;
            };
            if let Ok(hash) = hash_file(path, provider.kind().hash_kind()).await {
                if same_hash(&hash, &copy.base_hash) {
                    // Touched but not changed (an editor re-saving, say).
                    if let Ok((local_size, local_mtime_ns)) = snapshot(path) {
                        let _ = self.store().put_copy(
                            &cloud.remote,
                            &CopyRow {
                                local_size,
                                local_mtime_ns,
                                ..copy
                            },
                        );
                    }
                    return;
                }
            }
        }
        self.emit(EngineEvent::UploadNeeded {
            path: path.to_string_lossy().into_owned(),
            remote: cloud.remote,
            rel: cloud.rel,
        });
    }

    /// Looks for changes made while Kura wasn't running: edited copies and new
    /// files in remote folders.
    pub async fn scan_local_changes(&self) {
        let Ok(remotes) = self.store().remotes() else {
            return;
        };
        for remote in remotes {
            for file in files_under(&self.remote_root(&remote.name)).await {
                self.local_change(&file).await;
            }
        }
    }
}

/// Every regular file below `root`.
async fn files_under(root: &Path) -> Vec<PathBuf> {
    let root = root.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let mut files = Vec::new();
        let mut pending = vec![root];
        while let Some(directory) = pending.pop() {
            let Ok(entries) = std::fs::read_dir(&directory) else {
                continue;
            };
            for entry in entries.flatten() {
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                if file_type.is_dir() {
                    pending.push(entry.path());
                } else if file_type.is_file() {
                    files.push(entry.path());
                }
            }
        }
        files
    })
    .await
    .unwrap_or_default()
}
