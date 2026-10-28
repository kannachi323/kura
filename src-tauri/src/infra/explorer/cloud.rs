//! Cloud folders in the explorer. A cloud item's Kura path is its path under
//! `~/.kura/providers/<account>/`, so browsing, the transfer queue and undo
//! work with ordinary paths; this module routes those paths to Kuragine.
use kuragine::{
    CloudEntry, CloudPath, ConflictChoice, ConflictInfo, CopyState, Kuragine, UploadOutcome,
};

use super::*;
use crate::domain::explorer::{ClipboardOperation, CreateItemKind, ExplorerLocationKind};
use crate::infra::cloud::TransferProgress;

pub enum CloudUploadResult {
    Done,
    Conflict(ConflictInfo),
}

fn state_name(state: CopyState) -> &'static str {
    match state {
        CopyState::Online => "online",
        CopyState::Downloaded => "downloaded",
        CopyState::Modified => "modified",
        CopyState::Conflict => "conflict",
    }
}

fn leaf(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_owned()
}

impl ExplorerService {
    pub fn cloud(&self) -> &Kuragine {
        &self.cloud
    }

    /// The cloud account and remote path for a Kura path, if it is one.
    pub fn cloud_path(&self, path: &str) -> Option<CloudPath> {
        if path.contains("://") {
            return None;
        }
        self.cloud.resolve(Path::new(path))
    }

    pub(super) fn is_cloud_root(&self, path: &str) -> bool {
        Path::new(path) == self.mount_root
    }

    fn cloud_location(
        &self,
        cloud: Option<&CloudPath>,
        state: Option<CopyState>,
    ) -> ExplorerLocation {
        let provider_type = cloud
            .and_then(|cloud| self.cloud.remote(&cloud.remote).ok())
            .map(|remote| remote.kind.id().to_owned());
        ExplorerLocation {
            kind: ExplorerLocationKind::RemoteProvider,
            provider_type,
            remote_name: cloud.map(|cloud| cloud.remote.clone()),
            remote_path: cloud.map(|cloud| cloud.rel.clone()),
            cloud_state: state.map(|state| state_name(state).to_owned()),
            ..ExplorerLocation::default()
        }
    }

    /// `~/.kura/providers` lists the connected accounts.
    pub(super) fn cloud_root_listing(&self) -> ApiResult<DirectoryListing> {
        let entries: Vec<FileEntry> = self
            .cloud
            .remotes()?
            .into_iter()
            .map(|remote| {
                let cloud = CloudPath {
                    remote: remote.name.clone(),
                    rel: "/".to_owned(),
                };
                FileEntry {
                    id: remote.path.clone(),
                    name: remote.name.clone(),
                    path: remote.path.clone(),
                    extension: String::new(),
                    mime_type: None,
                    remote_modified: None,
                    kind: FileKind::Folder,
                    size_bytes: None,
                    modified_ms: None,
                    created_ms: None,
                    readonly: false,
                    hidden: false,
                    is_deleted: false,
                    location: self.cloud_location(Some(&cloud), None),
                }
            })
            .collect();
        Ok(DirectoryListing {
            path: display_path(&self.mount_root),
            title: Some("Cloud".to_owned()),
            parent_path: None,
            location: self.cloud_location(None, None),
            total_count: entries.len(),
            hidden_count: 0,
            entries,
            modified_ms: None,
            created_ms: None,
        })
    }

    pub(super) async fn cloud_listing(
        &self,
        cloud: CloudPath,
        show_hidden: bool,
        refresh: bool,
    ) -> ApiResult<DirectoryListing> {
        if refresh {
            self.cloud.refresh(&cloud.remote, &cloud.rel).await?;
        }
        let entries = self.cloud.list(&cloud.remote, &cloud.rel).await?;
        let total_count = entries.len();
        let hidden_count = entries
            .iter()
            .filter(|entry| entry.name.starts_with('.'))
            .count();
        let entries: Vec<FileEntry> = entries
            .into_iter()
            .filter(|entry| show_hidden || !entry.name.starts_with('.'))
            .map(|entry| self.cloud_entry(entry))
            .collect();
        let at_top = cloud.rel == kuragine::names::ROOT;
        let parent_path = if at_top {
            Some(display_path(&self.mount_root))
        } else {
            Some(display_path(
                &self
                    .cloud
                    .local(&cloud.remote, &kuragine::names::parent(&cloud.rel)),
            ))
        };
        Ok(DirectoryListing {
            path: display_path(&self.cloud.local(&cloud.remote, &cloud.rel)),
            title: at_top.then(|| cloud.remote.clone()),
            parent_path,
            location: self.cloud_location(Some(&cloud), None),
            entries,
            total_count,
            hidden_count,
            modified_ms: None,
            created_ms: None,
        })
    }

    fn cloud_entry(&self, entry: CloudEntry) -> FileEntry {
        let path = display_path(&entry.path);
        let cloud = CloudPath {
            remote: entry.remote.clone(),
            rel: entry.rel.clone(),
        };
        FileEntry {
            id: path.clone(),
            extension: if entry.is_dir {
                String::new()
            } else {
                Path::new(&entry.name)
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .unwrap_or_default()
                    .to_owned()
            },
            hidden: entry.name.starts_with('.'),
            name: entry.name,
            path,
            mime_type: entry.mime_type,
            remote_modified: None,
            kind: if entry.is_dir {
                FileKind::Folder
            } else {
                FileKind::File
            },
            size_bytes: (!entry.is_dir).then_some(entry.size),
            modified_ms: entry.modified_ms,
            created_ms: None,
            readonly: entry.read_only,
            is_deleted: false,
            location: self.cloud_location(Some(&cloud), (!entry.is_dir).then_some(entry.state)),
        }
    }

    pub(super) async fn cloud_is_directory(&self, cloud: &CloudPath) -> ApiResult<Option<bool>> {
        Ok(self
            .cloud
            .entry(&cloud.remote, &cloud.rel)
            .await?
            .map(|entry| entry.is_dir))
    }

    /// Downloads a cloud file if needed, recording the download in Transfers.
    pub(super) async fn cloud_ensure_local(
        &self,
        cloud: &CloudPath,
        cancellation: Option<&AtomicBool>,
    ) -> ApiResult<PathBuf> {
        let entry = self
            .cloud
            .entry(&cloud.remote, &cloud.rel)
            .await?
            .ok_or_else(|| ApiError::Message(format!("{} was not found.", cloud.rel)))?;
        if entry.is_dir || entry.state != CopyState::Online {
            let progress = TransferProgress::new(self.transfers.clone(), None, cancellation);
            return Ok(self
                .cloud
                .ensure_local(&cloud.remote, &cloud.rel, &progress)
                .await?);
        }
        let mut record = FileTransferRecord::new(
            FileTransferType::Download,
            FileTransferItemType::Remote,
            entry.name.clone(),
        );
        record.remote_source_name = cloud.remote.clone();
        record.remote_source_path = cloud.rel.clone();
        record.local_dest_path = display_path(&entry.path);
        record.total_bytes = entry.size.min(i64::MAX as u64) as i64;
        record.detail_message = "Downloading from the cloud".to_owned();
        let transfer_id = self.begin_transfer(record).await;
        let progress = TransferProgress::new(self.transfers.clone(), transfer_id, cancellation);
        let result = self
            .cloud
            .ensure_local(&cloud.remote, &cloud.rel, &progress)
            .await
            .map_err(ApiError::from);
        self.finish_transfer(transfer_id, result).await
    }

    fn cloud_record(
        &self,
        transfer_type: FileTransferType,
        name: &str,
        source: Option<(&str, Option<&CloudPath>)>,
        destination: Option<(&str, Option<&CloudPath>)>,
    ) -> FileTransferRecord {
        let mut record =
            FileTransferRecord::new(transfer_type, FileTransferItemType::Remote, name.to_owned());
        if let Some((path, cloud)) = source {
            record.local_source_path = path.to_owned();
            if let Some(cloud) = cloud {
                record.remote_source_name = cloud.remote.clone();
                record.remote_source_path = cloud.rel.clone();
            }
        }
        if let Some((path, cloud)) = destination {
            record.local_dest_path = path.to_owned();
            if let Some(cloud) = cloud {
                record.remote_dest_name = cloud.remote.clone();
                record.remote_dest_path = cloud.rel.clone();
            }
        }
        record
    }

    pub(super) async fn cloud_create(
        &self,
        directory: CloudPath,
        request: CreateItemRequest,
        cancellation: Option<&AtomicBool>,
        existing_transfer_id: Option<u64>,
    ) -> ApiResult<ExplorerOperationResult> {
        let destination = display_path(&self.cloud.local(
            &directory.remote,
            &kuragine::names::join(&directory.rel, &request.name),
        ));
        let transfer_id = match existing_transfer_id {
            Some(id) => Some(id),
            None => {
                let mut record = self.cloud_record(
                    FileTransferType::Create,
                    &request.name,
                    None,
                    Some((&destination, Some(&directory))),
                );
                record.detail_message = "Creating in the cloud".to_owned();
                self.begin_transfer(record).await
            }
        };
        let progress = TransferProgress::new(self.transfers.clone(), transfer_id, cancellation);
        let result = match request.kind {
            CreateItemKind::Folder => {
                self.cloud
                    .create_folder(&directory.remote, &directory.rel, &request.name)
                    .await
            }
            CreateItemKind::File => {
                self.cloud
                    .create_file(&directory.remote, &directory.rel, &request.name, &progress)
                    .await
            }
        }
        .map(|entry| ExplorerOperationResult {
            affected_paths: vec![display_path(&entry.path)],
            parent_path: Some(request.directory.clone()),
        })
        .map_err(ApiError::from);
        self.finish_transfer(transfer_id, result).await
    }

    pub(super) async fn cloud_rename(
        &self,
        cloud: CloudPath,
        request: &RenameItemRequest,
        existing_transfer_id: Option<u64>,
    ) -> ApiResult<ExplorerOperationResult> {
        let parent = Path::new(&request.path)
            .parent()
            .map(display_path)
            .unwrap_or_default();
        let transfer_id = match existing_transfer_id {
            Some(id) => Some(id),
            None => {
                let destination = display_path(&Path::new(&parent).join(&request.new_name));
                let mut record = self.cloud_record(
                    FileTransferType::Rename,
                    &request.new_name,
                    Some((&request.path, Some(&cloud))),
                    Some((&destination, Some(&cloud))),
                );
                record.detail_message = "Renaming in the cloud".to_owned();
                self.begin_transfer(record).await
            }
        };
        let result = self
            .cloud
            .rename(&cloud.remote, &cloud.rel, &request.new_name)
            .await
            .map(|rel| ExplorerOperationResult {
                affected_paths: vec![display_path(&self.cloud.local(&cloud.remote, &rel))],
                parent_path: Some(parent.clone()),
            })
            .map_err(ApiError::from);
        self.finish_transfer(transfer_id, result).await
    }

    pub(super) async fn cloud_delete(
        &self,
        path: &str,
        cloud: CloudPath,
        existing_transfer_id: Option<u64>,
    ) -> ApiResult<ExplorerOperationResult> {
        let transfer_id = match existing_transfer_id {
            Some(id) => Some(id),
            None => {
                let mut record = self.cloud_record(
                    FileTransferType::Delete,
                    &leaf(path),
                    Some((path, Some(&cloud))),
                    None,
                );
                record.detail_message = "Moving to the cloud's trash".to_owned();
                self.begin_transfer(record).await
            }
        };
        let result = self
            .cloud
            .trash(&cloud.remote, &cloud.rel)
            .await
            .map(|()| ExplorerOperationResult {
                affected_paths: vec![path.to_owned()],
                parent_path: Path::new(path).parent().map(display_path),
            })
            .map_err(ApiError::from);
        self.finish_transfer(transfer_id, result).await
    }

    pub(super) fn paste_touches_cloud(&self, request: &PasteItemsRequest) -> bool {
        self.cloud_path(&request.destination_directory).is_some()
            || self.is_cloud_root(&request.destination_directory)
            || request
                .sources
                .iter()
                .any(|source| self.cloud_path(&source.path).is_some())
    }

    /// Copies or moves items when the source or the destination is in the
    /// cloud: uploads, downloads, and copies within or between accounts.
    pub(super) async fn cloud_paste(
        &self,
        request: PasteItemsRequest,
        cancellation: Option<&AtomicBool>,
        existing_transfer_id: Option<u64>,
    ) -> ApiResult<ExplorerOperationResult> {
        if request.sources.is_empty() {
            return Err(ApiError::Message("Copy or cut an item first.".to_string()));
        }
        if self.is_cloud_root(&request.destination_directory) {
            return Err(ApiError::Message(
                "Open a cloud account's folder to paste there.".to_owned(),
            ));
        }
        let destination_cloud = self.cloud_path(&request.destination_directory);
        let single = request.sources.len() == 1;
        let mut affected_paths = Vec::new();
        for item in &request.sources {
            ensure_not_canceled_if(cancellation)?;
            let source_cloud = self.cloud_path(&item.path);
            let name = request
                .target_name
                .clone()
                .filter(|_| single)
                .unwrap_or_else(|| leaf(&item.path));
            let destination = display_path(&Path::new(&request.destination_directory).join(&name));
            let transfer_type = match (
                request.operation,
                source_cloud.is_some(),
                destination_cloud.is_some(),
            ) {
                (ClipboardOperation::Move, _, _) => FileTransferType::Move,
                (ClipboardOperation::Copy, true, false) => FileTransferType::Download,
                (ClipboardOperation::Copy, false, true) => FileTransferType::Upload,
                (ClipboardOperation::Copy, _, _) => FileTransferType::Copy,
            };
            let transfer_id = match existing_transfer_id {
                Some(id) => Some(id),
                None => {
                    let destination_target = destination_cloud.as_ref().map(|cloud| CloudPath {
                        remote: cloud.remote.clone(),
                        rel: kuragine::names::join(&cloud.rel, &name),
                    });
                    let mut record = self.cloud_record(
                        transfer_type,
                        &name,
                        Some((&item.path, source_cloud.as_ref())),
                        Some((&destination, destination_target.as_ref())),
                    );
                    record.total_bytes = item.size_bytes.unwrap_or_default().max(0);
                    record.detail_message = "Transferring with the cloud".to_owned();
                    self.begin_transfer(record).await
                }
            };
            let progress = TransferProgress::new(self.transfers.clone(), transfer_id, cancellation);
            let result = self
                .cloud_paste_one(
                    &item.path,
                    source_cloud,
                    destination_cloud.as_ref(),
                    &request.destination_directory,
                    &name,
                    request.operation,
                    &progress,
                    cancellation,
                )
                .await;
            affected_paths.push(self.finish_transfer(transfer_id, result).await?);
        }
        Ok(ExplorerOperationResult {
            affected_paths,
            parent_path: Some(request.destination_directory.clone()),
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn cloud_paste_one(
        &self,
        source_path: &str,
        source: Option<CloudPath>,
        destination: Option<&CloudPath>,
        destination_directory: &str,
        name: &str,
        operation: ClipboardOperation,
        progress: &TransferProgress<'_>,
        cancellation: Option<&AtomicBool>,
    ) -> ApiResult<String> {
        let moving = matches!(operation, ClipboardOperation::Move);
        match (source, destination) {
            (Some(source), Some(destination)) if source.remote == destination.remote => {
                let rel = if moving {
                    self.cloud
                        .move_to(&source.remote, &source.rel, &destination.rel, Some(name))
                        .await?
                } else {
                    self.cloud
                        .copy_within(
                            &source.remote,
                            &source.rel,
                            &destination.rel,
                            Some(name),
                            progress,
                        )
                        .await?
                };
                Ok(display_path(&self.cloud.local(&destination.remote, &rel)))
            }
            (Some(source), Some(destination)) => {
                // Between accounts: through a temporary folder.
                let staging = std::env::temp_dir()
                    .join("kura-cloud-transfer")
                    .join(uuid::Uuid::new_v4().to_string());
                let temporary = staging.join(name);
                let result = async {
                    self.cloud
                        .download_to(&source.remote, &source.rel, &temporary, progress)
                        .await?;
                    self.cloud
                        .upload_path(
                            &temporary,
                            &destination.remote,
                            &destination.rel,
                            Some(name),
                            progress,
                        )
                        .await
                }
                .await;
                let _ = tokio::fs::remove_dir_all(&staging).await;
                let rel = result?;
                if moving {
                    self.cloud.trash(&source.remote, &source.rel).await?;
                }
                Ok(display_path(&self.cloud.local(&destination.remote, &rel)))
            }
            (Some(source), None) => {
                let target = Path::new(destination_directory).join(name);
                self.cloud
                    .download_to(&source.remote, &source.rel, &target, progress)
                    .await?;
                if moving {
                    self.cloud.trash(&source.remote, &source.rel).await?;
                }
                Ok(display_path(&target))
            }
            (None, Some(destination)) => {
                let rel = self
                    .cloud
                    .upload_path(
                        Path::new(source_path),
                        &destination.remote,
                        &destination.rel,
                        Some(name),
                        progress,
                    )
                    .await?;
                if moving {
                    delete_local_path_cancellable(Path::new(source_path), cancellation).await?;
                }
                Ok(display_path(&self.cloud.local(&destination.remote, &rel)))
            }
            (None, None) => Err(ApiError::Message(format!(
                "{source_path} is not a cloud item."
            ))),
        }
    }

    /// Uploads a downloaded file that changed on disk, stopping at a conflict.
    pub async fn cloud_upload_change(
        &self,
        path: &str,
        cancellation: Option<&AtomicBool>,
        transfer_id: u64,
    ) -> ApiResult<CloudUploadResult> {
        let cloud = self
            .cloud_path(path)
            .ok_or_else(|| ApiError::Message(format!("{path} is not in a cloud folder.")))?;
        let progress = TransferProgress::new(
            self.transfers.clone(),
            nonzero_transfer_id(transfer_id),
            cancellation,
        );
        Ok(
            match self
                .cloud
                .upload_copy(&cloud.remote, &cloud.rel, &progress)
                .await?
            {
                UploadOutcome::Conflict(info) => CloudUploadResult::Conflict(info),
                _ => CloudUploadResult::Done,
            },
        )
    }

    /// Settles an upload conflict.
    pub async fn cloud_resolve_conflict(
        &self,
        path: &str,
        choice: ConflictChoice,
        cancellation: Option<&AtomicBool>,
        transfer_id: u64,
    ) -> ApiResult<CloudUploadResult> {
        let cloud = self
            .cloud_path(path)
            .ok_or_else(|| ApiError::Message(format!("{path} is not in a cloud folder.")))?;
        let progress = TransferProgress::new(
            self.transfers.clone(),
            nonzero_transfer_id(transfer_id),
            cancellation,
        );
        Ok(
            match self
                .cloud
                .resolve_conflict(&cloud.remote, &cloud.rel, choice, &progress)
                .await?
            {
                UploadOutcome::Conflict(info) => CloudUploadResult::Conflict(info),
                _ => CloudUploadResult::Done,
            },
        )
    }
}
