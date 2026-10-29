use super::*;

impl ExplorerService {
    pub async fn list_directory(
        &self,
        request: ListDirectoryRequest,
    ) -> ApiResult<DirectoryListing> {
        if let Some(path) = request.path.as_deref().map(str::trim) {
            if path == VIRTUAL_PATH_RECENT || path == VIRTUAL_PATH_STARRED {
                return self.list_library_virtual_directory(path).await;
            }
            if path == VIRTUAL_PATH_LIBRARY {
                return Ok(DirectoryListing {
                    path: path.to_owned(),
                    title: Some("Library".to_owned()),
                    parent_path: None,
                    location: ExplorerLocation::local(),
                    hidden_count: 0,
                    total_count: 0,
                    entries: Vec::new(),
                    modified_ms: None,
                    created_ms: None,
                });
            }
            if path == VIRTUAL_PATH_TRASH {
                return self.list_trash_virtual_directory().await;
            }
            if let Some(tag) = path.strip_prefix(VIRTUAL_PATH_TAG_PREFIX) {
                return self.list_tag_virtual_directory(path, tag).await;
            }
            if path.starts_with("kura://") {
                return Err(ApiError::Message(format!(
                    "Unsupported virtual Explorer location: {path}"
                )));
            }
            if self.is_cloud_root(path) {
                return self.cloud_root_listing();
            }
            if let Some(cloud) = self.cloud_path(path) {
                return self
                    .cloud_listing(
                        cloud,
                        request.show_hidden.unwrap_or(false),
                        request.force_remote_refresh.unwrap_or(false),
                    )
                    .await;
            }
        }

        let requested = request
            .path
            .as_deref()
            .filter(|path| !path.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| self.home_dir.clone());

        self.reject_virtual_mount_container(&display_path(&requested), "browse")?;
        let home_dir = self.home_dir.clone();
        tokio::task::spawn_blocking(move || list_directory(home_dir, request))
            .await
            .map_err(|err| ApiError::Message(format!("Explorer worker failed: {err}")))?
    }

    pub(super) async fn list_library_virtual_directory(
        &self,
        path: &str,
    ) -> ApiResult<DirectoryListing> {
        let snapshot = self.explorer_library.snapshot().await?;
        let source_items = if path == VIRTUAL_PATH_RECENT {
            snapshot.recent_files
        } else {
            snapshot.starred_files
        };
        let mut entries: Vec<FileEntry> = source_items
            .into_iter()
            .filter_map(|item| self.library_item_to_file_entry(item, path == VIRTUAL_PATH_RECENT))
            .collect();
        entries.sort_by(|left, right| {
            virtual_folder_rank(&left.kind)
                .cmp(&virtual_folder_rank(&right.kind))
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
        });
        let total_count = entries.len();
        Ok(DirectoryListing {
            path: path.to_string(),
            title: None,
            parent_path: None,
            location: ExplorerLocation::local(),
            hidden_count: 0,
            total_count,
            entries,
            modified_ms: None,
            created_ms: None,
        })
    }

    /// Items with a Finder tag, found by Spotlight as Finder's own Tags
    /// sidebar does. Folders Spotlight doesn't index won't appear.
    pub(super) async fn list_tag_virtual_directory(
        &self,
        path: &str,
        tag: &str,
    ) -> ApiResult<DirectoryListing> {
        let home = self.home_dir.clone();
        let name = percent_encoding::percent_decode_str(tag)
            .decode_utf8_lossy()
            .to_string();
        let tag_name = name.clone();
        let entries = tokio::task::spawn_blocking(move || -> ApiResult<Vec<FileEntry>> {
            let paths = tagged_paths(&home, &tag_name)?;
            let mut entries: Vec<FileEntry> = paths
                .iter()
                .filter_map(|path| path_entry(path, false))
                .collect();
            entries.sort_by(|left, right| {
                virtual_folder_rank(&left.kind)
                    .cmp(&virtual_folder_rank(&right.kind))
                    .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
            });
            Ok(entries)
        })
        .await
        .map_err(|err| ApiError::Message(format!("Tag search worker failed: {err}")))??;
        Ok(DirectoryListing {
            path: path.to_string(),
            title: Some(name),
            parent_path: None,
            location: ExplorerLocation::local(),
            hidden_count: 0,
            total_count: entries.len(),
            entries,
            modified_ms: None,
            created_ms: None,
        })
    }

    pub(super) async fn list_trash_virtual_directory(&self) -> ApiResult<DirectoryListing> {
        let trash_dir = self.trash_dir.clone();
        let ledger = self.trash_ledger.clone();
        let system_dirs = super::system_trash::system_trash_dirs(&self.home_dir);
        let entries = tokio::task::spawn_blocking(move || {
            trash_virtual_entries(&trash_dir, &system_dirs, &ledger.existing())
        })
        .await
        .map_err(|err| ApiError::Message(format!("Explorer trash worker failed: {err}")))??;
        let total_count = entries.len();
        Ok(DirectoryListing {
            path: VIRTUAL_PATH_TRASH.to_string(),
            title: Some("Trash".to_owned()),
            parent_path: None,
            location: ExplorerLocation::local(),
            hidden_count: 0,
            total_count,
            entries,
            modified_ms: None,
            created_ms: None,
        })
    }

    pub(super) fn library_item_to_file_entry(
        &self,
        item: ExplorerLibraryItem,
        prune_missing_local: bool,
    ) -> Option<FileEntry> {
        let path = item.path.trim();
        if path.is_empty() {
            return None;
        }
        if prune_missing_local && !Path::new(path).exists() {
            return None;
        }
        let path_buf = PathBuf::from(path);
        let name = if item.name.trim().is_empty() {
            path_buf
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or(path)
                .to_string()
        } else {
            item.name
        };
        let kind = if item.is_dir {
            FileKind::Folder
        } else {
            FileKind::File
        };
        let size_bytes = if item.size > 0 && !item.is_dir {
            Some(item.size as u64)
        } else {
            None
        };
        let mime_type = if item.mime_type.trim().is_empty() {
            None
        } else {
            Some(item.mime_type)
        };
        let remote_modified = if item.last_modified.trim().is_empty() {
            None
        } else {
            Some(item.last_modified)
        };
        Some(FileEntry {
            id: if item.id.trim().is_empty() {
                path.to_string()
            } else {
                item.id
            },
            name,
            path: path.to_string(),
            extension: path_buf
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_string(),
            mime_type,
            remote_modified,
            kind,
            size_bytes,
            modified_ms: None,
            created_ms: None,
            readonly: false,
            hidden: path_buf
                .file_name()
                .and_then(|value| value.to_str())
                .map(|value| value.starts_with('.'))
                .unwrap_or(false),
            is_deleted: false,
            location: ExplorerLocation::local(),
        })
    }

    pub async fn item_is_directory(&self, path: &str) -> ApiResult<Option<bool>> {
        if self.is_cloud_root(path) {
            return Ok(Some(true));
        }
        if let Some(cloud) = self.cloud_path(path) {
            return self.cloud_is_directory(&cloud).await;
        }
        self.reject_virtual_mount_container(path, "inspect")?;
        // Match std::filesystem::is_directory in the native Explorer: the
        // decision to browse or open follows a local symlink's target.
        match tokio::fs::metadata(path).await {
            Ok(metadata) => Ok(Some(metadata.is_dir())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(ApiError::Message(format!(
                "Failed to inspect {path}: {error}"
            ))),
        }
    }
}

/// Paths under `home` carrying the Finder tag `name`. Spotlight answers
/// this instantly, as for Finder's Tags; when it returns nothing (indexing
/// off, or a folder it skips) Kura scans Home itself.
#[cfg(target_os = "macos")]
fn tagged_paths(home: &Path, name: &str) -> ApiResult<Vec<PathBuf>> {
    let escaped = name.replace('\\', "\\\\").replace('"', "\\\"");
    let spotlight: Vec<PathBuf> = std::process::Command::new("mdfind")
        .arg("-onlyin")
        .arg(home)
        .arg(format!("kMDItemUserTags == \"{escaped}\""))
        .output()
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|line| !line.is_empty())
                .take(MAX_TAGGED)
                .map(PathBuf::from)
                .collect()
        })
        .unwrap_or_default();
    if !spotlight.is_empty() {
        return Ok(spotlight);
    }
    Ok(scan_for_tag(home, name))
}

#[cfg(not(target_os = "macos"))]
fn tagged_paths(_home: &Path, _name: &str) -> ApiResult<Vec<PathBuf>> {
    Ok(Vec::new())
}

const MAX_TAGGED: usize = 5_000;
/// The scan stops after this many items: about a second on a laptop SSD.
const MAX_SCANNED: usize = 250_000;

/// Walks Home (skipping hidden folders, app data and build output) and
/// checks each item's tags on several threads.
fn scan_for_tag(home: &Path, name: &str) -> Vec<PathBuf> {
    let exclusions = crate::infra::macos_privacy::BackgroundScanExclusions::new(home);
    let candidates: Vec<PathBuf> = walkdir::WalkDir::new(home)
        .max_depth(8)
        .into_iter()
        .filter_entry(|entry| {
            let file_name = entry.file_name().to_string_lossy();
            entry.depth() == 0
                || !(file_name.starts_with('.')
                    || matches!(
                        file_name.as_ref(),
                        "Library" | "node_modules" | "target" | "build" | "dist"
                    )
                    || exclusions
                        .roots()
                        .iter()
                        .any(|root| entry.path().starts_with(root)))
        })
        .filter_map(Result::ok)
        .filter(|entry| entry.depth() > 0)
        .take(MAX_SCANNED)
        .map(|entry| entry.into_path())
        .collect();
    let threads = std::thread::available_parallelism()
        .map(|count| count.get().min(8))
        .unwrap_or(4);
    let chunk = candidates.len().div_ceil(threads).max(1);
    let mut found: Vec<PathBuf> = std::thread::scope(|scope| {
        candidates
            .chunks(chunk)
            .map(|paths| {
                scope.spawn(move || {
                    paths
                        .iter()
                        .filter(|path| {
                            crate::domain::finder_tags::read_tags(path)
                                .iter()
                                .any(|tag| tag.name == name)
                        })
                        .cloned()
                        .collect::<Vec<_>>()
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .flat_map(|handle| handle.join().unwrap_or_default())
            .collect()
    });
    found.truncate(MAX_TAGGED);
    found
}

#[cfg(all(test, target_os = "macos"))]
mod tag_scan_tests {
    use super::*;

    #[test]
    fn the_fallback_scan_finds_tagged_items_and_skips_hidden_folders() {
        let home = tempfile::tempdir().expect("temp home");
        let docs = home.path().join("Documents");
        let hidden = home.path().join(".cache");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::create_dir_all(&hidden).unwrap();
        let tagged = docs.join("plan.txt");
        let hidden_tagged = hidden.join("plan.txt");
        for path in [&tagged, &hidden_tagged] {
            std::fs::write(path, b"x").unwrap();
            crate::domain::finder_tags::set_tag(&[path.display().to_string()], "Purple", true)
                .unwrap();
        }
        std::fs::write(docs.join("untagged.txt"), b"x").unwrap();

        assert_eq!(scan_for_tag(home.path(), "Purple"), vec![tagged]);
    }
}
