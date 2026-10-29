//! Moving items to the Trash, and putting them back.
//!
//! On macOS items go to the system Trash (the one Finder shows), so trashing
//! is a rename on the same volume. Kura cannot list `~/.Trash` without Full
//! Disk Access, so it records where each item it trashed came from. That
//! record drives Kura's Trash view and Put Back. Elsewhere items go to Kura's
//! own trash folder and are recorded the same way.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};

use crate::error::{ApiError, ApiResult};

#[derive(Clone)]
pub(super) struct TrashLedger {
    path: PathBuf,
    lock: Arc<Mutex<()>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct TrashRecord {
    trash_path: String,
    original_path: String,
}

impl TrashLedger {
    pub(super) fn new(path: PathBuf) -> Self {
        Self {
            path,
            lock: Arc::new(Mutex::new(())),
        }
    }

    pub(super) fn record(&self, trash_path: &Path, original_path: &Path) {
        let _guard = self.lock.lock();
        let mut records = self.load();
        let trash_path = trash_path.display().to_string();
        records.retain(|record| record.trash_path != trash_path);
        records.push(TrashRecord {
            trash_path,
            original_path: original_path.display().to_string(),
        });
        self.save(&records);
    }

    /// Recorded items still in the Trash. Drops records for items that were
    /// put back, emptied or removed by another app.
    pub(super) fn existing(&self) -> Vec<PathBuf> {
        let _guard = self.lock.lock();
        let records = self.load();
        let kept: Vec<TrashRecord> = records
            .iter()
            .filter(|record| Path::new(&record.trash_path).symlink_metadata().is_ok())
            .cloned()
            .collect();
        if kept.len() != records.len() {
            self.save(&kept);
        }
        kept.into_iter()
            .map(|record| PathBuf::from(record.trash_path))
            .collect()
    }

    pub(super) fn original_path(&self, trash_path: &Path) -> Option<PathBuf> {
        let _guard = self.lock.lock();
        let trash_path = trash_path.display().to_string();
        self.load()
            .into_iter()
            .find(|record| record.trash_path == trash_path)
            .map(|record| PathBuf::from(record.original_path))
    }

    fn load(&self) -> Vec<TrashRecord> {
        std::fs::read(&self.path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    fn save(&self, records: &[TrashRecord]) {
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(bytes) = serde_json::to_vec(records) {
            let temporary = self.path.with_extension("json.tmp");
            if std::fs::write(&temporary, bytes).is_ok() {
                let _ = std::fs::rename(&temporary, &self.path);
            }
        }
    }
}

/// Moves `source` to the system Trash and returns where it landed, or
/// `None` where there is no system Trash Kura can use.
#[cfg(target_os = "macos")]
pub(super) fn move_to_system_trash(source: &Path) -> ApiResult<Option<PathBuf>> {
    use objc2::rc::autoreleasepool;
    use objc2_foundation::{NSFileManager, NSString, NSURL};

    autoreleasepool(|_| {
        let path = NSString::from_str(&source.display().to_string());
        let url = NSURL::fileURLWithPath(&path);
        let mut resulting = None;
        NSFileManager::defaultManager()
            .trashItemAtURL_resultingItemURL_error(&url, Some(&mut resulting))
            .map_err(|error| {
                ApiError::Message(format!(
                    "Could not move {} to the Trash: {}",
                    source.display(),
                    error.localizedDescription()
                ))
            })?;
        Ok(resulting
            .and_then(|url| url.path())
            .map(|path| PathBuf::from(path.to_string())))
    })
}

#[cfg(not(target_os = "macos"))]
pub(super) fn move_to_system_trash(_source: &Path) -> ApiResult<Option<PathBuf>> {
    Ok(None)
}

/// The system's Trash folders for this user: `~/.Trash` and each mounted
/// volume's `.Trashes/<uid>`. Listing them needs Full Disk Access.
#[cfg(target_os = "macos")]
pub(super) fn system_trash_dirs(home: &Path) -> Vec<PathBuf> {
    // SAFETY: getuid has no preconditions.
    let uid = unsafe { libc::getuid() };
    let mut dirs = vec![home.join(".Trash")];
    if let Ok(volumes) = std::fs::read_dir("/Volumes") {
        dirs.extend(
            volumes
                .flatten()
                .map(|volume| volume.path().join(".Trashes").join(uid.to_string())),
        );
    }
    dirs
}

#[cfg(not(target_os = "macos"))]
pub(super) fn system_trash_dirs(_home: &Path) -> Vec<PathBuf> {
    Vec::new()
}

/// Where Finder says a trashed item came from, from the Trash folder's
/// `.DS_Store` (readable when Kura can read the Trash). Paths there are
/// relative to the volume: `/` for the startup disk, `/Volumes/<name>` for
/// the others.
fn finder_original_path(trash_path: &Path) -> Option<PathBuf> {
    let trash_dir = trash_path.parent()?;
    let trash_name = trash_path.file_name()?.to_str()?;
    let data = std::fs::read(trash_dir.join(".DS_Store")).ok()?;
    let record = crate::domain::ds_store::put_back_records(&data).remove(trash_name)?;
    let location = record.location?;
    let name = record.name.unwrap_or_else(|| trash_name.to_owned());
    let volume_root = volume_root_of(trash_dir);
    Some(
        volume_root
            .join(location.trim_start_matches('/'))
            .join(name),
    )
}

/// `/Volumes/<name>` for a path on another volume, else `/`.
fn volume_root_of(path: &Path) -> PathBuf {
    let mut components = path.components();
    match (components.next(), components.next(), components.next()) {
        (Some(std::path::Component::RootDir), Some(first), Some(second))
            if first.as_os_str() == "Volumes" =>
        {
            Path::new("/Volumes").join(second.as_os_str())
        }
        _ => PathBuf::from("/"),
    }
}

/// Moves a trashed item back to where it was trashed from. Fails rather
/// than replace something that now has its name, as Finder does.
pub(super) async fn put_back(ledger: &TrashLedger, trash_path: &Path) -> ApiResult<PathBuf> {
    let original = ledger
        .original_path(trash_path)
        .or_else(|| finder_original_path(trash_path))
        .ok_or_else(|| {
            ApiError::Message(format!(
                "Kura doesn't know where {} came from, so it can't put it back.",
                trash_path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_default()
            ))
        })?;
    if original.symlink_metadata().is_ok() {
        return Err(ApiError::Message(format!(
            "An item named {} already exists in {}.",
            original
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default(),
            original
                .parent()
                .map(|parent| parent.display().to_string())
                .unwrap_or_default()
        )));
    }
    if let Some(parent) = original.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|error| {
            ApiError::Message(format!("Could not recreate {}: {error}", parent.display()))
        })?;
    }
    if tokio::fs::rename(trash_path, &original).await.is_err() {
        // Kura's own trash folder can be on another volume.
        let cancellation = std::sync::atomic::AtomicBool::new(false);
        super::local_mutations::move_local_path_cancellable(trash_path, &original, &cancellation)
            .await?;
    }
    Ok(original)
}

impl super::ExplorerService {
    /// Trashes `source` and records where it came from.
    pub(super) async fn move_to_trash(
        &self,
        source: &Path,
        cancellation: Option<&std::sync::atomic::AtomicBool>,
    ) -> ApiResult<PathBuf> {
        let owned = source.to_path_buf();
        // Tests use Kura's own trash folder rather than the user's Trash.
        let system = if cfg!(test) {
            None
        } else {
            tokio::task::spawn_blocking(move || move_to_system_trash(&owned))
                .await
                .map_err(|error| ApiError::Message(format!("Trash worker failed: {error}")))??
        };
        let destination = match system {
            Some(destination) => destination,
            None => {
                super::local_mutations::trash_local_path_cancellable(
                    source,
                    &self.trash_dir,
                    cancellation,
                )
                .await?
            }
        };
        self.trash_ledger.record(&destination, source);
        Ok(destination)
    }

    /// Whether the Trash view sees everything in the system Trash, or only
    /// the items Kura put there (listing `~/.Trash` needs Full Disk Access).
    pub fn trash_fully_visible(&self) -> bool {
        match system_trash_dirs(&self.home_dir).first() {
            Some(trash) => std::fs::read_dir(trash).is_ok(),
            None => true,
        }
    }

    /// Puts trashed items back where they came from. Returns the folders
    /// that changed.
    pub async fn put_back_items(&self, paths: Vec<String>) -> ApiResult<Vec<String>> {
        let mut changed = Vec::new();
        let mut errors = Vec::new();
        for path in paths {
            match put_back(&self.trash_ledger, Path::new(&path)).await {
                Ok(original) => {
                    let _ = self.transfers.clear_trash_undo_for(path.clone()).await;
                    if let Some(parent) = original.parent() {
                        changed.push(parent.display().to_string());
                    }
                }
                Err(error) => errors.push(error.to_string()),
            }
        }
        if changed.is_empty() && !errors.is_empty() {
            return Err(ApiError::Message(errors.join("\n")));
        }
        changed.sort();
        changed.dedup();
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ledger_forgets_items_that_left_the_trash() {
        let dir = tempfile::tempdir().expect("temp dir");
        let ledger = TrashLedger::new(dir.path().join("ledger.json"));
        let kept = dir.path().join("kept.txt");
        let gone = dir.path().join("gone.txt");
        std::fs::write(&kept, b"x").expect("write");
        ledger.record(&kept, Path::new("/somewhere/kept.txt"));
        ledger.record(&gone, Path::new("/somewhere/gone.txt"));

        assert_eq!(ledger.existing(), vec![kept.clone()]);
        assert_eq!(
            ledger.original_path(&kept),
            Some(PathBuf::from("/somewhere/kept.txt"))
        );
        assert_eq!(ledger.original_path(&gone), None);
    }

    #[test]
    fn finder_records_give_the_original_path() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            dir.path().join(".DS_Store"),
            crate::domain::ds_store::tests::put_back_store(
                "report 2.pdf",
                "Users/me/Documents/",
                "report.pdf",
            ),
        )
        .expect("write");
        assert_eq!(
            finder_original_path(&dir.path().join("report 2.pdf")),
            Some(PathBuf::from("/Users/me/Documents/report.pdf"))
        );
        assert_eq!(finder_original_path(&dir.path().join("other.pdf")), None);
        assert_eq!(
            volume_root_of(Path::new("/Volumes/Backup/.Trashes/501")),
            PathBuf::from("/Volumes/Backup")
        );
    }

    #[tokio::test]
    async fn put_back_restores_the_original_path_and_refuses_to_replace() {
        let dir = tempfile::tempdir().expect("temp dir");
        let ledger = TrashLedger::new(dir.path().join("ledger.json"));
        let trashed = dir.path().join("trash").join("notes 2.txt");
        let original = dir.path().join("docs").join("notes.txt");
        std::fs::create_dir_all(trashed.parent().unwrap()).expect("mkdir");
        std::fs::write(&trashed, b"notes").expect("write");
        ledger.record(&trashed, &original);

        assert_eq!(
            put_back(&ledger, &trashed).await.expect("put back"),
            original
        );
        assert_eq!(std::fs::read(&original).expect("read"), b"notes");

        std::fs::write(&trashed, b"again").expect("write");
        assert!(put_back(&ledger, &trashed).await.is_err());
        assert!(trashed.exists());
    }
}
