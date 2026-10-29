//! Changes made from Kura's own UI. These are explicit cloud operations, unlike
//! edits from other apps, which only ever upload content.
use std::path::{Path, PathBuf};

use super::{
    browse::{local_state, make_writable, prune_empty_dirs},
    now_ms, CloudEntry, Kuragine, UploadOutcome,
};
use crate::{
    cloud::{RemoteItem, TransferObserver, UploadTarget},
    hash::same_hash,
    names,
    store::ItemRow,
    Error, Result,
};

/// The cloud name for a local name: Google exports lose the extension Kura
/// added for them.
fn remote_name(display: &str, item: &RemoteItem) -> String {
    match item.export_extension() {
        Some(extension) => display
            .strip_suffix(&format!(".{extension}"))
            .unwrap_or(display)
            .to_owned(),
        None => display.to_owned(),
    }
}

fn validate_new_name(name: &str) -> Result<()> {
    if name.trim().is_empty() || name == "." || name == ".." || name.contains('/') {
        return Err(Error::invalid(format!("“{name}” isn't a valid name.")));
    }
    Ok(())
}

/// Adds progress across the files of a folder operation.
struct Running<'a> {
    observer: &'a dyn TransferObserver,
    done: std::sync::atomic::AtomicU64,
}

impl<'a> Running<'a> {
    fn new(observer: &'a dyn TransferObserver) -> Self {
        Self {
            observer,
            done: Default::default(),
        }
    }

    fn file(&self) -> FileProgress<'_> {
        FileProgress {
            running: self,
            current: Default::default(),
        }
    }
}

struct FileProgress<'a> {
    running: &'a Running<'a>,
    current: std::sync::atomic::AtomicU64,
}

impl TransferObserver for FileProgress<'_> {
    fn progress(&self, done: u64, _total: u64) {
        use std::sync::atomic::Ordering;
        let before = self.current.swap(done, Ordering::Relaxed);
        let total = self
            .running
            .done
            .fetch_add(done.saturating_sub(before), Ordering::Relaxed)
            + done.saturating_sub(before);
        self.running.observer.progress(total, total);
    }

    fn is_canceled(&self) -> bool {
        self.running.observer.is_canceled()
    }
}

impl Kuragine {
    /// Fails if `rel` is taken by an item other than `except_id`.
    fn ensure_free(&self, remote: &str, rel: &str, except_id: Option<&str>) -> Result<()> {
        if let Some(row) = self.store().item(remote, rel)? {
            if except_id != Some(row.item.id.as_str()) {
                return Err(Error::invalid(format!(
                    "“{}” already exists there.",
                    names::file_name(rel)
                )));
            }
        }
        Ok(())
    }

    async fn listed_folder(&self, remote: &str, rel: &str) -> Result<ItemRow> {
        let folder = self.item_row(remote, rel).await?;
        if !folder.item.is_dir {
            return Err(Error::invalid(format!(
                "{} is not a folder.",
                names::file_name(rel)
            )));
        }
        if folder.listed_ms == 0 {
            self.refresh_children(remote, &folder).await?;
        }
        Ok(folder)
    }

    pub async fn create_folder(
        &self,
        remote: &str,
        parent_rel: &str,
        name: &str,
    ) -> Result<CloudEntry> {
        let remote = self.remote_row(remote)?.name;
        let result = async {
            validate_new_name(name)?;
            let parent = self.listed_folder(&remote, parent_rel).await?;
            let rel = names::join(parent_rel, &names::safe_local_name(name));
            self.ensure_free(&remote, &rel, None)?;
            let provider = self.provider(&remote).await?;
            let created = provider.create_folder(&parent.item, name).await?;
            let row = ItemRow {
                path: rel.clone(),
                parent_path: parent.path.clone(),
                item: created,
                listed_ms: now_ms(),
            };
            self.store().put_item(&remote, &row)?;
            tokio::fs::create_dir_all(self.local(&remote, &rel)).await?;
            self.emit_listing(&remote, parent_rel);
            self.to_entry(&remote, &row)
        }
        .await;
        self.noted(&remote, result)
    }

    /// Creates an empty file in the cloud (and its local copy).
    pub async fn create_file(
        &self,
        remote: &str,
        parent_rel: &str,
        name: &str,
        observer: &dyn TransferObserver,
    ) -> Result<CloudEntry> {
        validate_new_name(name)?;
        let remote = self.remote_row(remote)?.name;
        let rel = names::join(parent_rel, &names::safe_local_name(name));
        self.listed_folder(&remote, parent_rel).await?;
        self.ensure_free(&remote, &rel, None)?;
        let local = self.local(&remote, &rel);
        if tokio::fs::metadata(&local).await.is_ok() {
            return Err(Error::invalid(format!("“{name}” already exists there.")));
        }
        if let Some(parent) = local.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&local, b"").await?;
        match self.upload_copy(&remote, &rel, observer).await {
            Ok(UploadOutcome::Uploaded | UploadOutcome::Unchanged) => {}
            Ok(_) => return Err(Error::invalid(format!("“{name}” could not be created."))),
            Err(error) => {
                let _ = tokio::fs::remove_file(&local).await;
                return Err(error);
            }
        }
        let row = self
            .store()
            .item(&remote, &rel)?
            .ok_or_else(|| Error::NotFound(name.to_owned()))?;
        self.to_entry(&remote, &row)
    }

    pub async fn rename(&self, remote: &str, rel: &str, new_name: &str) -> Result<String> {
        let parent = names::parent(rel);
        self.move_to(remote, rel, &parent, Some(new_name)).await
    }

    /// Moves (and optionally renames) an item within its remote. Returns the
    /// new remote path.
    pub async fn move_to(
        &self,
        remote: &str,
        rel: &str,
        destination_rel: &str,
        new_name: Option<&str>,
    ) -> Result<String> {
        let remote = self.remote_row(remote)?.name;
        let result = async {
            if rel == names::ROOT {
                return Err(Error::invalid(
                    "A cloud account's top folder can't be moved.",
                ));
            }
            if names::is_within(destination_rel, rel) {
                return Err(Error::invalid("A folder can't be moved into itself."));
            }
            let row = self.item_row(&remote, rel).await?;
            let display = new_name
                .map(names::safe_local_name)
                .unwrap_or_else(|| names::file_name(rel).to_owned());
            validate_new_name(&display)?;
            let new_rel = names::join(destination_rel, &display);
            if new_rel == rel {
                return Ok(new_rel);
            }
            let old_parent_rel = names::parent(rel);
            let moving = destination_rel != old_parent_rel;
            let destination = if moving {
                Some(self.listed_folder(&remote, destination_rel).await?)
            } else {
                None
            };
            let old_parent = if moving {
                Some(self.item_row(&remote, &old_parent_rel).await?)
            } else {
                None
            };
            self.ensure_free(&remote, &new_rel, Some(&row.item.id))?;
            let cloud_name = match new_name {
                Some(name) => remote_name(name, &row.item),
                None => row.item.name.clone(),
            };
            let _guard = self.lock(&remote, rel).await;
            let provider = self.provider(&remote).await?;
            let updated = provider
                .rename_move(
                    &row.item,
                    &cloud_name,
                    destination
                        .as_ref()
                        .zip(old_parent.as_ref())
                        .map(|(to, from)| (&to.item, &from.item)),
                )
                .await?;
            let from = self.local(&remote, rel);
            let to = self.local(&remote, &new_rel);
            self.store().move_subtree(&remote, rel, &new_rel)?;
            self.store().put_item(
                &remote,
                &ItemRow {
                    path: new_rel.clone(),
                    parent_path: destination_rel.to_owned(),
                    item: updated,
                    listed_ms: row.listed_ms,
                },
            )?;
            let case_only = rel.to_lowercase() == new_rel.to_lowercase();
            if !case_only && tokio::fs::symlink_metadata(&to).await.is_ok() {
                // A stray local file sits where the item is going.
                self.set_aside(&remote, &new_rel).await?;
            }
            if tokio::fs::symlink_metadata(&from).await.is_ok() {
                if let Some(parent) = to.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                tokio::fs::rename(&from, &to).await?;
            }
            self.emit_listing(&remote, &old_parent_rel);
            if moving {
                self.emit_listing(&remote, destination_rel);
            }
            Ok(new_rel)
        }
        .await;
        self.noted(&remote, result)
    }

    /// Copies an item within its remote: files server-side, folders by
    /// recreating them. Returns the new remote path.
    pub async fn copy_within(
        &self,
        remote: &str,
        rel: &str,
        destination_rel: &str,
        new_name: Option<&str>,
        observer: &dyn TransferObserver,
    ) -> Result<String> {
        let remote = self.remote_row(remote)?.name;
        let result = async {
            if names::is_within(destination_rel, rel) {
                return Err(Error::invalid("A folder can't be copied into itself."));
            }
            let row = self.item_row(&remote, rel).await?;
            let display = new_name
                .map(names::safe_local_name)
                .unwrap_or_else(|| names::file_name(rel).to_owned());
            let cloud_name = match new_name {
                Some(name) => remote_name(name, &row.item),
                None => row.item.name.clone(),
            };
            self.copy_item(
                &remote,
                &row,
                destination_rel,
                &display,
                &cloud_name,
                observer,
            )
            .await
        }
        .await;
        self.noted(&remote, result)
    }

    async fn copy_item(
        &self,
        remote: &str,
        row: &ItemRow,
        destination_rel: &str,
        display: &str,
        cloud_name: &str,
        observer: &dyn TransferObserver,
    ) -> Result<String> {
        crate::cloud::ensure_running(observer)?;
        let destination = self.listed_folder(remote, destination_rel).await?;
        let new_rel = names::join(destination_rel, display);
        self.ensure_free(remote, &new_rel, None)?;
        let provider = self.provider(remote).await?;
        if !row.item.is_dir {
            let copied = provider
                .copy_file(&row.item, &destination.item, cloud_name)
                .await?;
            self.store().put_item(
                remote,
                &ItemRow {
                    path: new_rel.clone(),
                    parent_path: destination_rel.to_owned(),
                    item: copied,
                    listed_ms: 0,
                },
            )?;
            observer.progress(row.item.size, row.item.size);
            self.emit_listing(remote, destination_rel);
            return Ok(new_rel);
        }
        let created = provider
            .create_folder(&destination.item, cloud_name)
            .await?;
        self.store().put_item(
            remote,
            &ItemRow {
                path: new_rel.clone(),
                parent_path: destination_rel.to_owned(),
                item: created,
                listed_ms: now_ms(),
            },
        )?;
        self.listed_folder(remote, &row.path).await?;
        for child in self.store().children(remote, &row.path)? {
            let child_display = names::file_name(&child.path).to_owned();
            let child_name = child.item.name.clone();
            Box::pin(self.copy_item(
                remote,
                &child,
                &new_rel,
                &child_display,
                &child_name,
                observer,
            ))
            .await?;
        }
        self.emit_listing(remote, destination_rel);
        Ok(new_rel)
    }

    /// Moves an item to the provider's trash and removes its local copies.
    /// Local edits that weren't uploaded are moved to the conflicts folder.
    pub async fn trash(&self, remote: &str, rel: &str) -> Result<()> {
        let remote = self.remote_row(remote)?.name;
        let result = async {
            if rel == names::ROOT {
                return Err(Error::invalid(
                    "A cloud account's top folder can't be deleted.",
                ));
            }
            let row = self.item_row(&remote, rel).await?;
            let _guard = self.lock(&remote, rel).await;
            let provider = self.provider(&remote).await?;
            match provider.trash(&row.item).await {
                Ok(()) => {}
                Err(error) if error.is_not_found() => {}
                Err(error) => return Err(error),
            }
            self.remove_local_tree(&remote, rel).await?;
            self.store().delete_subtree(&remote, rel)?;
            self.emit_listing(&remote, &names::parent(rel));
            Ok(())
        }
        .await;
        self.noted(&remote, result)
    }

    async fn remove_local_tree(&self, remote: &str, rel: &str) -> Result<()> {
        let local = self.local(remote, rel);
        let Ok(metadata) = tokio::fs::symlink_metadata(&local).await else {
            return Ok(());
        };
        if metadata.is_dir() {
            let mut entries = tokio::fs::read_dir(&local).await?;
            while let Some(entry) = entries.next_entry().await? {
                let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                    continue;
                };
                Box::pin(self.remove_local_tree(remote, &names::join(rel, &name))).await?;
            }
            prune_empty_dirs(&local).await;
            return Ok(());
        }
        let unchanged = self
            .store()
            .copy(remote, rel)?
            .is_some_and(|copy| local_state(&local, &copy) == Some(false));
        if unchanged || names::is_scratch_name(names::file_name(rel)) {
            let _ = make_writable(&local).await;
            tokio::fs::remove_file(&local).await?;
        } else {
            self.set_aside(remote, rel).await?;
        }
        Ok(())
    }

    /// Uploads a local file or folder from outside the providers folder into
    /// a cloud folder. An existing file of the same name is replaced.
    pub async fn upload_path(
        &self,
        source: &Path,
        remote: &str,
        destination_rel: &str,
        name: Option<&str>,
        observer: &dyn TransferObserver,
    ) -> Result<String> {
        let remote = self.remote_row(remote)?.name;
        let running = Running::new(observer);
        let result = self
            .upload_tree(source, &remote, destination_rel, name, &running)
            .await;
        self.noted(&remote, result)
    }

    async fn upload_tree(
        &self,
        source: &Path,
        remote: &str,
        destination_rel: &str,
        name: Option<&str>,
        running: &Running<'_>,
    ) -> Result<String> {
        crate::cloud::ensure_running(running.observer)?;
        let name = match name {
            Some(name) => name.to_owned(),
            None => source
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| Error::invalid(format!("Can't upload {}.", source.display())))?
                .to_owned(),
        };
        validate_new_name(&name)?;
        let destination = self.listed_folder(remote, destination_rel).await?;
        let rel = names::join(destination_rel, &names::safe_local_name(&name));
        let existing = self.store().item(remote, &rel)?;
        let provider = self.provider(remote).await?;
        let metadata = tokio::fs::metadata(source).await?;
        if metadata.is_dir() {
            let folder = match existing {
                Some(row) if row.item.is_dir => row,
                Some(_) => return Err(Error::invalid(format!("“{name}” is a file in the cloud."))),
                None => {
                    let created = provider.create_folder(&destination.item, &name).await?;
                    let row = ItemRow {
                        path: rel.clone(),
                        parent_path: destination_rel.to_owned(),
                        item: created,
                        listed_ms: now_ms(),
                    };
                    self.store().put_item(remote, &row)?;
                    row
                }
            };
            let mut entries = tokio::fs::read_dir(source).await?;
            while let Some(entry) = entries.next_entry().await? {
                let file_type = entry.file_type().await?;
                if !(file_type.is_dir() || file_type.is_file()) {
                    continue;
                }
                let child_name = entry.file_name().to_string_lossy().into_owned();
                if file_type.is_file() && names::is_scratch_name(&child_name) {
                    continue;
                }
                Box::pin(self.upload_tree(&entry.path(), remote, &folder.path, None, running))
                    .await?;
            }
            self.emit_listing(remote, destination_rel);
            return Ok(rel);
        }

        if existing.as_ref().is_some_and(|row| row.item.is_dir) {
            return Err(Error::invalid(format!(
                "“{name}” is a folder in the cloud."
            )));
        }
        let target = UploadTarget {
            parent: &destination.item,
            name: &name,
            existing: existing.as_ref().map(|row| (&row.item, None)),
        };
        let uploaded = provider.upload(target, source, &running.file()).await?;
        self.store().put_item(
            remote,
            &ItemRow {
                path: rel.clone(),
                parent_path: destination_rel.to_owned(),
                item: uploaded,
                listed_ms: 0,
            },
        )?;
        self.emit_listing(remote, destination_rel);
        Ok(rel)
    }

    /// Saves a cloud file or folder to a local path outside the providers
    /// folder, reusing unchanged local copies.
    pub async fn download_to(
        &self,
        remote: &str,
        rel: &str,
        destination: &Path,
        observer: &dyn TransferObserver,
    ) -> Result<()> {
        let remote = self.remote_row(remote)?.name;
        let running = Running::new(observer);
        let result = self
            .download_tree(&remote, rel, destination, &running)
            .await;
        self.noted(&remote, result)
    }

    async fn download_tree(
        &self,
        remote: &str,
        rel: &str,
        destination: &Path,
        running: &Running<'_>,
    ) -> Result<()> {
        crate::cloud::ensure_running(running.observer)?;
        let row = self.item_row(remote, rel).await?;
        if row.item.is_dir {
            tokio::fs::create_dir_all(destination).await?;
            self.listed_folder(remote, rel).await?;
            for child in self.store().children(remote, rel)? {
                let name = names::file_name(&child.path).to_owned();
                Box::pin(self.download_tree(remote, &child.path, &destination.join(name), running))
                    .await?;
            }
            return Ok(());
        }
        let local = self.local(remote, rel);
        let reusable = self
            .store()
            .copy(remote, rel)?
            .filter(|copy| copy.id == row.item.id)
            .filter(|copy| local_state(&local, copy) == Some(false))
            .filter(|copy| {
                row.item
                    .hash
                    .as_deref()
                    .is_none_or(|hash| same_hash(hash, &copy.base_hash))
            })
            .is_some();
        if reusable {
            if let Some(parent) = destination.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::copy(&local, destination).await?;
            running.file().progress(row.item.size, row.item.size);
            return Ok(());
        }
        let partial = partial_path(destination);
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let provider = self.provider(remote).await?;
        let downloaded = match provider
            .download(&row.item, &partial, &running.file())
            .await
        {
            Ok(downloaded) => downloaded,
            Err(error) => {
                let _ = tokio::fs::remove_file(&partial).await;
                return Err(error);
            }
        };
        if let Some(expected) = row.item.hash.as_deref() {
            if !same_hash(expected, &downloaded.hash) {
                let _ = tokio::fs::remove_file(&partial).await;
                return Err(Error::Network(format!(
                    "{} didn't match the cloud's checksum after downloading. Try again.",
                    row.item.name
                )));
            }
        }
        tokio::fs::rename(&partial, destination).await?;
        Ok(())
    }
}

fn partial_path(destination: &Path) -> PathBuf {
    let name = destination
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    destination.with_file_name(format!(".{name}.kura-partial"))
}
