use std::{
    ffi::OsStr,
    fs,
    path::{Component, Path, PathBuf},
    sync::{mpsc, Mutex as StdMutex},
    time::UNIX_EPOCH,
};

use serde::{Deserialize, Serialize};

use crate::error::{ApiError, ApiResult};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListDirectoryRequest {
    pub path: Option<String>,
    pub show_hidden: Option<bool>,
    #[serde(default)]
    pub force_remote_refresh: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateItemRequest {
    pub directory: String,
    pub name: String,
    pub kind: CreateItemKind,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreateItemKind {
    File,
    Folder,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameItemRequest {
    pub path: String,
    pub new_name: String,
    pub source_is_directory: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameItemsRequest {
    pub items: Vec<RenameItemRequest>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteItemsRequest {
    pub paths: Vec<String>,
    #[serde(default = "default_delete_permanent")]
    pub permanent: bool,
}

fn default_delete_permanent() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteItemsRequest {
    pub sources: Vec<PasteItem>,
    pub destination_directory: String,
    pub operation: ClipboardOperation,
    #[serde(default)]
    pub target_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteTextRequest {
    pub destination_directory: String,
    pub text: String,
    #[serde(default)]
    pub preferred_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteBlobRequest {
    pub destination_directory: String,
    pub bytes: Vec<u8>,
    #[serde(default)]
    pub preferred_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteItem {
    pub path: String,
    pub is_directory: bool,
    #[serde(default)]
    pub size_bytes: Option<i64>,
    #[serde(default)]
    pub remote_modified: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareOpenItemRequest {
    pub path: String,
    pub size_bytes: Option<i64>,
    pub remote_modified: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareDragItemsRequest {
    pub items: Vec<PrepareDragItemRequest>,
    pub session_id: Option<String>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareDragItemRequest {
    pub path: String,
    pub is_directory: bool,
    pub size_bytes: Option<i64>,
    pub remote_modified: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedOpenItem {
    pub local_path: String,
    pub cached: bool,
    pub source_path: Option<String>,
    pub cache_path: Option<String>,
    pub cache_hit: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedDragItemsResult {
    pub items: Vec<PreparedDragItem>,
    pub skipped: Vec<PreparedDragSkippedItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedDragItem {
    pub source_path: String,
    pub local_path: String,
    pub is_directory: bool,
    pub cached: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedDragSkippedItem {
    pub source_path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerPreviewPayload {
    pub mime_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedImageThumbnail {
    pub path: String,
    pub mime_type: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipboardOperation {
    Copy,
    Move,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerOperationResult {
    pub affected_paths: Vec<String>,
    pub parent_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryListing {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub parent_path: Option<String>,
    pub location: ExplorerLocation,
    pub entries: Vec<FileEntry>,
    pub total_count: usize,
    pub hidden_count: usize,
    pub modified_ms: Option<i64>,
    pub created_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub id: String,
    pub name: String,
    pub path: String,
    pub extension: String,
    pub mime_type: Option<String>,
    pub remote_modified: Option<String>,
    pub kind: FileKind,
    pub size_bytes: Option<u64>,
    pub modified_ms: Option<i64>,
    pub created_ms: Option<i64>,
    pub readonly: bool,
    pub hidden: bool,
    #[serde(default, rename = "isDeleted")]
    pub is_deleted: bool,
    pub location: ExplorerLocation,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerLocation {
    pub kind: ExplorerLocationKind,
    // Unset fields are left out: a 100k-file listing repeats this per entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_device_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_root_id: Option<String>,
    /// For a connected device: whether it lets this device change its files.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_writable: Option<bool>,
    /// For a cloud item: "online", "downloaded", "modified" or "conflict".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloud_state: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExplorerLocationKind {
    #[default]
    Local,
    RemoteProvider,
    Remote,
    PeerDevice,
}

impl ExplorerLocation {
    pub fn local() -> Self {
        Self::default()
    }

    pub fn peer_device(
        device_id: String,
        root_id: String,
        relative_path: String,
        writable: bool,
    ) -> Self {
        Self {
            kind: ExplorerLocationKind::PeerDevice,
            provider_type: None,
            remote_name: None,
            remote_path: Some(relative_path),
            peer_device_id: Some(device_id),
            peer_root_id: Some(root_id),
            peer_writable: Some(writable),
            cloud_state: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    Folder,
    File,
    Symlink,
    Other,
}

pub fn list_directory(
    home_dir: PathBuf,
    request: ListDirectoryRequest,
) -> ApiResult<DirectoryListing> {
    let requested = request
        .path
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or(home_dir);
    let path = requested
        .canonicalize()
        .unwrap_or_else(|_| requested.clone());
    let show_hidden = request.show_hidden.unwrap_or(false);
    let read_dir = fs::read_dir(&path).map_err(|err| {
        ApiError::Message(format!(
            "Failed to list directory {}: {err}",
            path.display()
        ))
    })?;

    // Entry paths are this folder's path plus the name, as `Path::join` would
    // build them. Joining onto the canonical form is slow on Windows, where
    // it is a verbatim `\?\` path, so the string is built directly.
    let mut path_prefix = display_path(&path);
    if !path_prefix.ends_with(std::path::is_separator) {
        path_prefix.push(std::path::MAIN_SEPARATOR);
    }
    let (mut entries, total_count, hidden_count) =
        scan_entries(read_dir, show_hidden, &path_prefix);
    // One lowercase key per entry, not two allocations per comparison.
    entries.sort_by_cached_key(|entry| (folder_rank(&entry.kind), entry.name.to_lowercase()));

    let metadata = fs::symlink_metadata(&path).ok();
    Ok(DirectoryListing {
        parent_path: path.parent().map(display_path),
        path: display_path(&path),
        title: None,
        location: ExplorerLocation::local(),
        entries,
        total_count,
        hidden_count,
        modified_ms: metadata
            .as_ref()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(system_time_ms),
        created_ms: metadata
            .as_ref()
            .and_then(|metadata| metadata.created().ok())
            .and_then(system_time_ms),
    })
}

pub fn paste_items(request: PasteItemsRequest) -> ApiResult<ExplorerOperationResult> {
    if request.sources.is_empty() {
        return Err(ApiError::Message("Copy or cut an item first.".to_string()));
    }

    let destination_directory = normalize_existing_dir(&request.destination_directory)?;
    let target_name = if request.sources.len() == 1 {
        request
            .target_name
            .as_deref()
            .map(validate_file_name)
            .transpose()?
    } else {
        None
    };
    let mut affected_paths = Vec::new();

    for raw_source in request.sources {
        let source = PathBuf::from(&raw_source.path);
        if !source.exists() {
            return Err(ApiError::Message(format!(
                "{} does not exist.",
                source.display()
            )));
        }

        let file_name = target_name
            .map(OsStr::new)
            .or_else(|| source.file_name())
            .ok_or_else(|| ApiError::Message(format!("Cannot paste {}.", source.display())))?;
        let destination = destination_directory.join(file_name);
        if destination.exists() {
            return Err(ApiError::Message(format!(
                "{} already exists.",
                display_path(&destination)
            )));
        }

        if source.is_dir() && destination_directory.starts_with(&source) {
            return Err(ApiError::Message(
                "Cannot paste a folder into itself.".to_string(),
            ));
        }

        match request.operation {
            ClipboardOperation::Copy => copy_path(&source, &destination)?,
            ClipboardOperation::Move => move_path(&source, &destination)?,
        }

        affected_paths.push(display_path(&destination));
    }

    Ok(ExplorerOperationResult {
        affected_paths,
        parent_path: Some(display_path(&destination_directory)),
    })
}

fn normalize_existing_dir(path: &str) -> ApiResult<PathBuf> {
    let path = PathBuf::from(path);
    let canonical = path.canonicalize().map_err(|err| {
        ApiError::Message(format!(
            "Failed to resolve directory {}: {err}",
            path.display()
        ))
    })?;
    if !canonical.is_dir() {
        return Err(ApiError::Message(format!(
            "{} is not a directory.",
            canonical.display()
        )));
    }
    Ok(canonical)
}

fn validate_file_name(name: &str) -> ApiResult<&str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(ApiError::Message("Enter a name.".to_string()));
    }
    let path = Path::new(trimmed);
    let mut components = path.components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Ok(trimmed),
        _ => Err(ApiError::Message(
            "Names cannot contain path separators.".to_string(),
        )),
    }
}

/// Below this many entries the per-file stat calls are cheaper than threads.
const PARALLEL_STAT_THRESHOLD: usize = 4096;
/// Entries handed to a stat worker at a time.
const STAT_BATCH: usize = 2048;
/// lstat stops scaling past a handful of threads (kernel vnode locks).
const MAX_STAT_THREADS: usize = 8;

/// Reads a directory and stats each visible entry. Returns the entries, the
/// total count and the hidden count.
///
/// One `lstat` per file dominates a large listing, so once a folder passes
/// the threshold the calls run on a few worker threads while this thread
/// keeps reading names.
fn scan_entries(
    read_dir: fs::ReadDir,
    show_hidden: bool,
    path_prefix: &str,
) -> (Vec<FileEntry>, usize, usize) {
    let mut total_count = 0usize;
    let mut hidden_count = 0usize;
    let mut visible = read_dir.flatten().filter_map(|item| {
        total_count += 1;
        let name = item.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            hidden_count += 1;
            if !show_hidden {
                return None;
            }
        }
        Some((item, name))
    });
    let stat = |(item, name): (fs::DirEntry, String)| file_entry(&item, name, path_prefix);

    let first: Vec<_> = visible.by_ref().take(PARALLEL_STAT_THRESHOLD).collect();
    let threads = std::thread::available_parallelism()
        .map_or(1, |count| count.get())
        .min(MAX_STAT_THREADS);
    let entries = if first.len() < PARALLEL_STAT_THRESHOLD || threads < 2 {
        first
            .into_iter()
            .chain(visible.by_ref())
            .filter_map(stat)
            .collect()
    } else {
        let (sender, receiver) = mpsc::channel::<Vec<(fs::DirEntry, String)>>();
        let receiver = StdMutex::new(receiver);
        std::thread::scope(|scope| {
            let workers: Vec<_> = (0..threads)
                .map(|_| {
                    scope.spawn(|| {
                        let mut entries = Vec::new();
                        loop {
                            let batch = match receiver.lock() {
                                Ok(receiver) => receiver.recv(),
                                Err(_) => break,
                            };
                            let Ok(batch) = batch else { break };
                            entries.extend(batch.into_iter().filter_map(stat));
                        }
                        entries
                    })
                })
                .collect();
            let mut batch = first;
            loop {
                let rest = batch.split_off(batch.len().min(STAT_BATCH));
                let _ = sender.send(batch);
                batch = rest;
                if batch.is_empty() {
                    batch = visible.by_ref().take(STAT_BATCH).collect();
                    if batch.is_empty() {
                        break;
                    }
                }
            }
            drop(sender);
            workers
                .into_iter()
                .flat_map(|worker| worker.join().unwrap_or_default())
                .collect()
        })
    };
    drop(visible);
    (entries, total_count, hidden_count)
}

/// `path_prefix` is the folder path ending in a separator.
fn file_entry(item: &fs::DirEntry, name: String, path_prefix: &str) -> Option<FileEntry> {
    // DirEntry::metadata does not follow symlinks, like symlink_metadata.
    let metadata = item.metadata().ok()?;
    let file_type = metadata.file_type();
    let kind = if file_type.is_symlink() {
        FileKind::Symlink
    } else if file_type.is_dir() {
        FileKind::Folder
    } else if file_type.is_file() {
        FileKind::File
    } else {
        FileKind::Other
    };
    let extension = Path::new(&name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_owned();
    let mut path = String::with_capacity(path_prefix.len() + name.len());
    path.push_str(path_prefix);
    path.push_str(&name);
    Some(FileEntry {
        id: path.clone(),
        extension,
        mime_type: None,
        remote_modified: None,
        hidden: name.starts_with('.'),
        name,
        path,
        kind,
        size_bytes: if metadata.is_file() {
            Some(metadata.len())
        } else {
            None
        },
        modified_ms: metadata.modified().ok().and_then(system_time_ms),
        created_ms: metadata.created().ok().and_then(system_time_ms),
        readonly: metadata.permissions().readonly(),
        is_deleted: false,
        location: ExplorerLocation::local(),
    })
}

fn copy_path(source: &Path, destination: &Path) -> ApiResult<()> {
    let metadata = fs::symlink_metadata(source).map_err(|err| {
        ApiError::Message(format!("Failed to inspect {}: {err}", source.display()))
    })?;
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        copy_directory(source, destination)
    } else {
        fs::copy(source, destination).map_err(|err| {
            ApiError::Message(format!(
                "Failed to copy {} to {}: {err}",
                source.display(),
                destination.display()
            ))
        })?;
        Ok(())
    }
}

fn copy_directory(source: &Path, destination: &Path) -> ApiResult<()> {
    fs::create_dir(destination).map_err(|err| {
        ApiError::Message(format!(
            "Failed to create folder {}: {err}",
            destination.display()
        ))
    })?;

    for entry in fs::read_dir(source).map_err(|err| {
        ApiError::Message(format!("Failed to read folder {}: {err}", source.display()))
    })? {
        let entry = entry.map_err(|err| {
            ApiError::Message(format!("Failed to read folder {}: {err}", source.display()))
        })?;
        let child_source = entry.path();
        let child_destination = destination.join(entry.file_name());
        copy_path(&child_source, &child_destination)?;
    }

    Ok(())
}

fn move_path(source: &Path, destination: &Path) -> ApiResult<()> {
    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(rename_error) => {
            copy_path(source, destination)?;
            let metadata = fs::symlink_metadata(source).map_err(|err| {
                ApiError::Message(format!("Failed to inspect {}: {err}", source.display()))
            })?;
            if metadata.is_dir() && !metadata.file_type().is_symlink() {
                fs::remove_dir_all(source).map_err(|err| {
                    ApiError::Message(format!(
                        "Moved copy to {}, but failed to remove source {} after rename failed ({rename_error}): {err}",
                        destination.display(),
                        source.display()
                    ))
                })?;
            } else {
                fs::remove_file(source).map_err(|err| {
                    ApiError::Message(format!(
                        "Moved copy to {}, but failed to remove source {} after rename failed ({rename_error}): {err}",
                        destination.display(),
                        source.display()
                    ))
                })?;
            }
            Ok(())
        }
    }
}

fn system_time_ms(time: std::time::SystemTime) -> Option<i64> {
    let duration = time.duration_since(UNIX_EPOCH).ok()?;
    duration.as_millis().try_into().ok()
}

fn folder_rank(kind: &FileKind) -> u8 {
    match kind {
        FileKind::Folder => 0,
        FileKind::Symlink => 1,
        FileKind::File => 2,
        FileKind::Other => 3,
    }
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(dir: &Path, show_hidden: bool) -> DirectoryListing {
        list_directory(
            PathBuf::new(),
            ListDirectoryRequest {
                path: Some(dir.display().to_string()),
                show_hidden: Some(show_hidden),
                force_remote_refresh: None,
            },
        )
        .expect("list directory")
    }

    #[test]
    fn large_folders_list_every_entry_folders_first() {
        let dir = tempfile::tempdir().expect("temp dir");
        let files = PARALLEL_STAT_THRESHOLD + STAT_BATCH + 7;
        for index in 0..files {
            fs::write(dir.path().join(format!("File {index:05}.txt")), b"x").expect("file");
        }
        for index in 0..3 {
            fs::write(dir.path().join(format!(".hidden-{index}")), b"").expect("hidden");
        }
        fs::create_dir(dir.path().join("zeta")).expect("folder");

        let listing = list(dir.path(), false);
        assert_eq!(listing.total_count, files + 4);
        assert_eq!(listing.hidden_count, 3);
        assert_eq!(listing.entries.len(), files + 1);
        assert_eq!(listing.entries[0].name, "zeta");
        assert!(matches!(listing.entries[0].kind, FileKind::Folder));
        assert_eq!(listing.entries[1].name, "File 00000.txt");
        assert_eq!(listing.entries[1].size_bytes, Some(1));
        assert!(listing.entries[1..]
            .windows(2)
            .all(|pair| pair[0].name.to_lowercase() <= pair[1].name.to_lowercase()));

        assert_eq!(list(dir.path(), true).entries.len(), files + 4);
    }

    #[test]
    fn small_folders_keep_symlinks_unresolved() {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::write(dir.path().join("a.txt"), b"abc").expect("file");
        #[cfg(unix)]
        std::os::unix::fs::symlink(dir.path().join("a.txt"), dir.path().join("link")).unwrap();

        let listing = list(dir.path(), false);
        let file = listing.entries.iter().find(|entry| entry.name == "a.txt");
        assert_eq!(file.and_then(|entry| entry.size_bytes), Some(3));
        #[cfg(unix)]
        assert!(listing
            .entries
            .iter()
            .any(|entry| entry.name == "link" && matches!(entry.kind, FileKind::Symlink)));
    }
}
