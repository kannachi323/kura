//! Watches the folders open in Explorer panes so a listing refreshes when
//! another app adds, removes or renames something in it, as Finder does.
//!
//! Each folder is watched non-recursively. Bursts of changes are coalesced per
//! folder so a large copy into an open folder costs one refresh, not thousands.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant},
};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::error::{ApiError, ApiResult};

pub const DIRECTORY_CHANGED_EVENT: &str = "kura://directory-changed";

/// Quiet time after the last change before a folder is reported.
const DEBOUNCE: Duration = Duration::from_millis(250);
/// A folder that never goes quiet is still reported this often.
const MAX_DELAY: Duration = Duration::from_secs(1);

#[derive(Clone, Default)]
pub struct DirectoryWatchService {
    state: Arc<Mutex<Option<WatchState>>>,
}

struct WatchState {
    watcher: RecommendedWatcher,
    watched: HashSet<PathBuf>,
    /// The folders each window has open; the watcher covers their union.
    wanted_by: HashMap<String, HashSet<PathBuf>>,
}

impl DirectoryWatchService {
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts the watcher. `on_change` receives each batch of changed folders.
    pub fn start<F>(&self, on_change: F) -> ApiResult<()>
    where
        F: Fn(Vec<String>) + Send + 'static,
    {
        let (sender, receiver) = mpsc::channel::<PathBuf>();
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            let Ok(event) = event else { return };
            if matches!(event.kind, EventKind::Access(_)) {
                return;
            }
            for path in event.paths {
                // A change to a child refreshes the folder that lists it; a
                // change to the folder itself (renamed, deleted) refreshes it.
                if let Some(parent) = path.parent() {
                    let _ = sender.send(parent.to_path_buf());
                }
                let _ = sender.send(path);
            }
        })
        .map_err(|error| ApiError::Message(format!("Could not watch folders: {error}")))?;
        std::thread::Builder::new()
            .name("kura-directory-watch".into())
            .spawn(move || coalesce(receiver, on_change))
            .map_err(|error| ApiError::Message(format!("Could not watch folders: {error}")))?;
        *self.lock()? = Some(WatchState {
            watcher,
            watched: HashSet::new(),
            wanted_by: HashMap::new(),
        });
        Ok(())
    }

    /// Replaces the folders `owner` (a window) has open with `paths`. Paths
    /// that are not local folders (virtual or cloud locations) are ignored.
    pub fn set_directories(&self, owner: &str, paths: Vec<String>) -> ApiResult<()> {
        let mut guard = self.lock()?;
        let Some(state) = guard.as_mut() else {
            return Ok(());
        };
        let owned: HashSet<PathBuf> = paths
            .into_iter()
            .map(PathBuf::from)
            .filter(|path| path.is_absolute() && path.is_dir())
            .collect();
        if owned.is_empty() {
            state.wanted_by.remove(owner);
        } else {
            state.wanted_by.insert(owner.to_owned(), owned);
        }
        let wanted: HashSet<PathBuf> = state.wanted_by.values().flatten().cloned().collect();
        for path in state
            .watched
            .difference(&wanted)
            .cloned()
            .collect::<Vec<_>>()
        {
            let _ = state.watcher.unwatch(&path);
            state.watched.remove(&path);
        }
        for path in wanted {
            if state.watched.contains(&path) {
                continue;
            }
            if state
                .watcher
                .watch(&path, RecursiveMode::NonRecursive)
                .is_ok()
            {
                state.watched.insert(path);
            }
        }
        Ok(())
    }

    fn lock(&self) -> ApiResult<std::sync::MutexGuard<'_, Option<WatchState>>> {
        self.state
            .lock()
            .map_err(|_| ApiError::Message("Folder watcher lock was poisoned.".into()))
    }
}

/// Collects changed folders until they go quiet, then reports them once.
fn coalesce<F>(receiver: mpsc::Receiver<PathBuf>, on_change: F)
where
    F: Fn(Vec<String>),
{
    while let Ok(first) = receiver.recv() {
        let started = Instant::now();
        let mut changed = HashSet::from([first]);
        loop {
            let remaining = MAX_DELAY.saturating_sub(started.elapsed());
            match receiver.recv_timeout(DEBOUNCE.min(remaining)) {
                Ok(path) => {
                    changed.insert(path);
                    if started.elapsed() >= MAX_DELAY {
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
        on_change(changed.iter().map(|path| display(path)).collect());
    }
}

fn display(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_a_change_in_a_watched_folder_once() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().canonicalize().expect("canonical temp dir");
        let (sender, receiver) = mpsc::channel();
        let service = DirectoryWatchService::new();
        service
            .start(move |paths| {
                let _ = sender.send(paths);
            })
            .expect("start watcher");
        service
            .set_directories("main", vec![root.display().to_string()])
            .expect("watch");
        // FSEvents can take a moment to start delivering.
        std::thread::sleep(Duration::from_millis(300));
        for index in 0..20 {
            std::fs::write(root.join(format!("file-{index}.txt")), b"x").expect("write");
        }

        let root_text = root.display().to_string();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut batches = 0;
        let mut saw_root = false;
        while Instant::now() < deadline && !saw_root {
            if let Ok(paths) = receiver.recv_timeout(Duration::from_millis(200)) {
                batches += 1;
                saw_root = paths.contains(&root_text);
            }
        }
        assert!(saw_root, "the watched folder was not reported");
        assert!(batches <= 3, "20 writes produced {batches} batches");
    }

    #[test]
    fn a_folder_stays_watched_while_any_window_has_it_open() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().canonicalize().expect("canonical temp dir");
        let service = DirectoryWatchService::new();
        service.start(|_| {}).expect("start watcher");
        let path = vec![root.display().to_string()];
        service
            .set_directories("main", path.clone())
            .expect("watch");
        service.set_directories("files-1", path).expect("watch");
        let watched = |service: &DirectoryWatchService| {
            service
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .watched
                .contains(&root)
        };

        service
            .set_directories("main", Vec::new())
            .expect("unwatch");
        assert!(watched(&service), "the other window still has it open");
        service
            .set_directories("files-1", Vec::new())
            .expect("unwatch");
        assert!(!watched(&service));
    }
}
