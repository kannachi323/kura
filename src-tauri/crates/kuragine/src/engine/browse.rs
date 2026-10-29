//! Listing folders: answered from the index, refreshed from the provider.
use std::{collections::HashMap, path::Path};

use super::{now_ms, CloudEntry, CloudPath, CopyState, Kuragine};
use crate::{
    cloud::RemoteItem,
    names,
    store::{CopyRow, ItemRow},
    Error, Result,
};

/// A listing older than this is refreshed in the background when shown.
const STALE_MS: i64 = 15_000;

/// The local name for a cloud item: made safe, plus the export extension for
/// Google documents. Collisions are resolved afterwards.
pub(crate) fn display_name(item: &RemoteItem) -> String {
    let name = names::safe_local_name(&item.name);
    match item.export_extension() {
        Some(extension)
            if !name
                .to_ascii_lowercase()
                .ends_with(&format!(".{extension}")) =>
        {
            format!("{name}.{extension}")
        }
        _ => name,
    }
}

impl Kuragine {
    /// A folder's entries. Served from the index when it has been listed
    /// before (and refreshed in the background if stale); otherwise listed now.
    pub async fn list(&self, remote: &str, rel: &str) -> Result<Vec<CloudEntry>> {
        let remote = self.remote_row(remote)?.name;
        let result = self.list_inner(&remote, rel).await;
        self.noted(&remote, result)
    }

    async fn list_inner(&self, remote: &str, rel: &str) -> Result<Vec<CloudEntry>> {
        let folder = self.item_row(remote, rel).await?;
        if !folder.item.is_dir {
            return Err(Error::invalid(format!(
                "{} is not a folder.",
                names::file_name(rel)
            )));
        }
        if folder.listed_ms == 0 {
            self.refresh_children(remote, &folder).await?;
        } else if now_ms() - folder.listed_ms > STALE_MS {
            self.spawn_refresh(remote, &folder.path);
        }
        self.entries(remote, rel)
    }

    /// Lists a folder from the provider now.
    pub async fn refresh(&self, remote: &str, rel: &str) -> Result<()> {
        let remote = self.remote_row(remote)?.name;
        let result = async {
            let folder = self.item_row(&remote, rel).await?;
            self.refresh_children(&remote, &folder).await.map(|_| ())
        }
        .await;
        self.noted(&remote, result)
    }

    /// One entry, resolving its folders as needed. `None` if it doesn't exist.
    pub async fn entry(&self, remote: &str, rel: &str) -> Result<Option<CloudEntry>> {
        let remote = self.remote_row(remote)?.name;
        match self.item_row(&remote, rel).await {
            Ok(row) => Ok(Some(self.to_entry(&remote, &row)?)),
            Err(error) if error.is_not_found() => Ok(None),
            Err(error) => Err(self.note_auth(error, &remote)),
        }
    }

    pub(crate) fn entries(&self, remote: &str, rel: &str) -> Result<Vec<CloudEntry>> {
        self.store()
            .children(remote, rel)?
            .iter()
            .map(|row| self.to_entry(remote, row))
            .collect()
    }

    pub(crate) fn to_entry(&self, remote: &str, row: &ItemRow) -> Result<CloudEntry> {
        let path = self.local(remote, &row.path);
        let state = if row.item.is_dir {
            if path.is_dir() {
                CopyState::Downloaded
            } else {
                CopyState::Online
            }
        } else if self.store().conflict(remote, &row.path)?.is_some() {
            CopyState::Conflict
        } else {
            match self.store().copy(remote, &row.path)? {
                Some(copy) => match local_state(&path, &copy) {
                    Some(true) => CopyState::Modified,
                    Some(false) => CopyState::Downloaded,
                    None => CopyState::Online,
                },
                None if path.is_file() => CopyState::Modified,
                None => CopyState::Online,
            }
        };
        Ok(CloudEntry {
            remote: remote.to_owned(),
            rel: row.path.clone(),
            name: names::file_name(&row.path).to_owned(),
            is_dir: row.item.is_dir,
            size: row.item.size,
            modified_ms: row.item.modified_ms,
            mime_type: row.item.mime_type.clone(),
            read_only: row.item.export_mime.is_some(),
            state,
            path,
        })
    }

    /// The index row for a remote path, listing folders on the way if needed.
    pub(crate) async fn item_row(&self, remote: &str, rel: &str) -> Result<ItemRow> {
        if rel == names::ROOT {
            return self.root_row(remote).await;
        }
        if let Some(row) = self.store().item(remote, rel)? {
            return Ok(row);
        }
        let parent = Box::pin(self.item_row(remote, &names::parent(rel))).await?;
        if !parent.item.is_dir {
            return Err(Error::NotFound(rel.to_owned()));
        }
        self.refresh_children(remote, &parent).await?;
        self.store()
            .item(remote, rel)?
            .ok_or_else(|| Error::NotFound(names::file_name(rel).to_owned()))
    }

    async fn root_row(&self, remote: &str) -> Result<ItemRow> {
        if let Some(row) = self.store().item(remote, names::ROOT)? {
            return Ok(row);
        }
        let provider = self.provider(remote).await?;
        let row = ItemRow {
            path: names::ROOT.to_owned(),
            parent_path: String::new(),
            item: provider.root().await?,
            listed_ms: 0,
        };
        self.store().put_item(remote, &row)?;
        Ok(row)
    }

    pub(crate) fn spawn_refresh(&self, remote: &str, rel: &str) {
        let key = CloudPath {
            remote: remote.to_owned(),
            rel: rel.to_owned(),
        };
        if !self.inner.refreshing.lock().unwrap().insert(key.clone()) {
            return;
        }
        let engine = self.clone();
        tokio::spawn(async move {
            let result = async {
                let folder = engine.item_row(&key.remote, &key.rel).await?;
                engine.refresh_children(&key.remote, &folder).await
            }
            .await;
            if let Err(error) = result {
                engine.note_auth(error, &key.remote);
            }
            engine.inner.refreshing.lock().unwrap().remove(&key);
        });
    }

    /// Lists a folder from the provider and updates the index. Files that
    /// vanished lose their unchanged local copies; items renamed in the cloud
    /// take their local copies along. Returns whether anything changed.
    pub(crate) async fn refresh_children(&self, remote: &str, folder: &ItemRow) -> Result<bool> {
        let provider = self.provider(remote).await?;
        let items = provider.list(&folder.item).await?;
        let wanted: Vec<String> = items.iter().map(display_name).collect();
        let keys: Vec<String> = items.iter().map(|item| item.id.clone()).collect();
        let unique = names::unique_names(&wanted, &keys);
        let rows: Vec<ItemRow> = items
            .into_iter()
            .zip(unique)
            .map(|(item, name)| ItemRow {
                path: names::join(&folder.path, &name),
                parent_path: folder.path.clone(),
                item,
                listed_ms: 0,
            })
            .collect();

        let before = self.store().children(remote, &folder.path)?;
        let changed = before.len() != rows.len()
            || rows.iter().any(|row| {
                !before.iter().any(|old| {
                    old.path == row.path
                        && old.item.id == row.item.id
                        && old.item.revision == row.item.revision
                        && old.item.size == row.item.size
                })
            });
        if changed {
            self.follow_cloud_renames(remote, &before, &rows).await?;
        }
        let removed = self
            .store()
            .replace_children(remote, &folder.path, &rows, now_ms())?;
        for path in removed {
            let copies = self.store().delete_subtree(remote, &path)?;
            self.drop_unchanged_copies(remote, &copies).await;
            prune_empty_dirs(&self.local(remote, &path)).await;
        }
        if changed {
            self.emit_listing(remote, &folder.path);
        }
        Ok(changed)
    }

    /// When an item kept its ID but got a new path, moves its local copy (and
    /// index rows) so the download isn't lost.
    async fn follow_cloud_renames(
        &self,
        remote: &str,
        before: &[ItemRow],
        rows: &[ItemRow],
    ) -> Result<()> {
        let previous: HashMap<&str, &ItemRow> = before
            .iter()
            .map(|row| (row.item.id.as_str(), row))
            .collect();
        for row in rows {
            let Some(old) = previous.get(row.item.id.as_str()) else {
                continue;
            };
            if old.path == row.path || before.iter().any(|other| other.path == row.path) {
                continue;
            }
            let from = self.local(remote, &old.path);
            let to = self.local(remote, &row.path);
            if tokio::fs::symlink_metadata(&to).await.is_ok() {
                continue;
            }
            if tokio::fs::symlink_metadata(&from).await.is_ok() {
                tokio::fs::rename(&from, &to).await?;
            }
            self.store().move_subtree(remote, &old.path, &row.path)?;
        }
        Ok(())
    }

    /// Deletes local copies whose cloud items are gone, unless they were
    /// changed locally (those stay where they are).
    pub(crate) async fn drop_unchanged_copies(&self, remote: &str, copies: &[CopyRow]) {
        for copy in copies {
            let path = self.local(remote, &copy.path);
            if local_state(&path, copy) == Some(false) {
                let _ = make_writable(&path).await;
                let _ = tokio::fs::remove_file(&path).await;
            }
        }
    }
}

/// `Some(true)` if the local file differs from what Kuragine left there,
/// `Some(false)` if unchanged, `None` if it's gone.
pub(crate) fn local_state(path: &Path, copy: &CopyRow) -> Option<bool> {
    let (size, mtime) = snapshot(path).ok()?;
    Some(size != copy.local_size || mtime != copy.local_mtime_ns)
}

pub(crate) fn snapshot(path: &Path) -> std::io::Result<(u64, i64)> {
    let metadata = std::fs::metadata(path)?;
    let modified = metadata
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos().min(i64::MAX as u128) as i64)
        .unwrap_or_default();
    Ok((metadata.len(), modified))
}

pub(crate) async fn make_writable(path: &Path) -> std::io::Result<()> {
    let metadata = tokio::fs::metadata(path).await?;
    let mut permissions = metadata.permissions();
    if permissions.readonly() {
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        tokio::fs::set_permissions(path, permissions).await?;
    }
    Ok(())
}

/// Removes empty folders at and below `path`.
pub(crate) async fn prune_empty_dirs(path: &Path) {
    let path = path.to_path_buf();
    let _ = tokio::task::spawn_blocking(move || prune(&path)).await;

    fn prune(path: &Path) -> bool {
        let Ok(entries) = std::fs::read_dir(path) else {
            return false;
        };
        let mut empty = true;
        for entry in entries.flatten() {
            let child = entry.path();
            if !(child.is_dir() && prune(&child)) {
                empty = false;
            }
        }
        empty && std::fs::remove_dir(path).is_ok()
    }
}
