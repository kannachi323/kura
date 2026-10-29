//! Keeps the search index current from file-system events, so a created,
//! renamed or deleted file is searchable right away instead of at the next
//! periodic scan. That incremental scan stays as the safety net for anything
//! the events miss.
//!
//! Each root is watched recursively. Events are coalesced until they go quiet
//! and applied as one path-level update, so a large copy costs a few commits,
//! not one per file.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex, Weak},
    time::{Duration, Instant},
};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use super::{SearchInner, SearchScanRequest, SearchService, SearchWatchReport};
use crate::error::{ApiError, ApiResult};

/// Quiet time after the last event before a batch is applied.
const DEBOUNCE: Duration = Duration::from_millis(100);
/// A batch that never goes quiet is still applied this often.
const MAX_DELAY: Duration = Duration::from_secs(1);
/// While a scan holds the index, pending changes are retried this often.
const RETRY: Duration = Duration::from_millis(500);

pub(super) struct IndexWatch {
    _watcher: RecommendedWatcher,
    roots: Vec<PathBuf>,
    request: Arc<Mutex<SearchScanRequest>>,
}

enum Signal {
    Changed(PathBuf),
    /// Events below this folder were dropped; it has to be walked.
    Rescan(PathBuf),
}

impl IndexWatch {
    pub(super) fn start(
        service: Weak<SearchInner>,
        request: SearchScanRequest,
        roots: Vec<PathBuf>,
        index_root: PathBuf,
    ) -> ApiResult<Self> {
        let failed = |error: &dyn std::fmt::Display| {
            ApiError::Message(format!("Could not watch indexed folders: {error}"))
        };
        let paths = PathMap::new(&roots, &index_root);
        let (sender, receiver) = mpsc::channel::<Signal>();
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                let Ok(event) = event else { return };
                if matches!(event.kind, EventKind::Access(_)) {
                    return;
                }
                let rescan = event.need_rescan();
                if rescan && event.paths.is_empty() {
                    for root in paths.roots() {
                        let _ = sender.send(Signal::Rescan(root.clone()));
                    }
                }
                for path in event.paths.iter().filter_map(|path| paths.indexed(path)) {
                    let _ = sender.send(if rescan {
                        Signal::Rescan(path)
                    } else {
                        Signal::Changed(path)
                    });
                }
            })
            .map_err(|error| failed(&error))?;
        for root in &roots {
            // A root that doesn't exist (yet) is caught by the periodic scan.
            if root.is_dir() {
                watcher
                    .watch(root, RecursiveMode::Recursive)
                    .map_err(|error| failed(&error))?;
            }
        }
        let request = Arc::new(Mutex::new(request));
        let shared = request.clone();
        std::thread::Builder::new()
            .name("kura-search-watch".into())
            .spawn(move || apply_batches(receiver, service, shared))
            .map_err(|error| failed(&error))?;
        Ok(Self {
            _watcher: watcher,
            roots,
            request,
        })
    }

    pub(super) fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    /// Takes new filters or depth for the same roots.
    pub(super) fn set_request(&self, request: SearchScanRequest) {
        if let Ok(mut current) = self.request.lock() {
            *current = request;
        }
    }
}

/// Collects events until they go quiet, then applies them as one update.
/// Ends when the watcher is dropped or the search service is gone.
fn apply_batches(
    receiver: mpsc::Receiver<Signal>,
    service: Weak<SearchInner>,
    request: Arc<Mutex<SearchScanRequest>>,
) {
    let mut changed = BTreeSet::new();
    let mut rescan = BTreeSet::new();
    let mut first_event = None;
    loop {
        let waiting = changed.is_empty() && rescan.is_empty();
        let first = if waiting {
            match receiver.recv() {
                Ok(signal) => Some(signal),
                Err(_) => return,
            }
        } else {
            match receiver.recv_timeout(RETRY) {
                Ok(signal) => Some(signal),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        };
        if let Some(signal) = first {
            let started = Instant::now();
            first_event.get_or_insert(started);
            let mut add = |signal| match signal {
                Signal::Changed(path) => changed.insert(path),
                Signal::Rescan(path) => rescan.insert(path),
            };
            add(signal);
            loop {
                let remaining = MAX_DELAY.saturating_sub(started.elapsed());
                if remaining.is_zero() {
                    break;
                }
                match receiver.recv_timeout(DEBOUNCE.min(remaining)) {
                    Ok(signal) => {
                        add(signal);
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => break,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }
        }
        let Some(inner) = service.upgrade() else {
            return;
        };
        let service = SearchService { inner };
        let Ok(request) = request.lock().map(|request| request.clone()) else {
            return;
        };
        let applying = Instant::now();
        let paths: Vec<PathBuf> = changed.iter().cloned().collect();
        let folders: Vec<PathBuf> = rescan.iter().cloned().collect();
        match service.apply_path_changes(&request, &paths, &folders) {
            Ok(Some(changes)) => {
                service.report_watch_update(&SearchWatchReport {
                    changes,
                    first_event: first_event.unwrap_or(applying),
                    applying,
                    committed: Instant::now(),
                });
            }
            // A scan holds the index; keep the batch and try again.
            Ok(None) => continue,
            // The periodic scan reconciles whatever this missed.
            Err(error) => eprintln!("kura: search index update failed: {error}"),
        }
        changed.clear();
        rescan.clear();
        first_event = None;
    }
}

/// Maps event paths, which FSEvents reports with symbolic links resolved
/// (`/private/var/...`), back to the form the roots were indexed under.
struct PathMap {
    roots: Vec<(PathBuf, PathBuf)>,
    ignored: Vec<PathBuf>,
}

impl PathMap {
    fn new(roots: &[PathBuf], index_root: &Path) -> Self {
        let canonical = |path: &Path| path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        Self {
            roots: roots
                .iter()
                .map(|root| (canonical(root), root.clone()))
                .collect(),
            // The index's own writes would otherwise wake the watcher after
            // every commit.
            ignored: vec![index_root.to_path_buf(), canonical(index_root)],
        }
    }

    fn roots(&self) -> impl Iterator<Item = &PathBuf> {
        self.roots.iter().map(|(_, indexed)| indexed)
    }

    fn indexed(&self, path: &Path) -> Option<PathBuf> {
        if self.ignored.iter().any(|ignored| path.starts_with(ignored)) {
            return None;
        }
        self.roots.iter().find_map(|(canonical, indexed)| {
            if let Ok(relative) = path.strip_prefix(canonical) {
                Some(indexed.join(relative))
            } else if path.starts_with(indexed) {
                Some(path.to_path_buf())
            } else {
                None
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_paths_map_back_to_the_indexed_root() {
        let map = PathMap {
            roots: vec![(
                PathBuf::from("/private/var/folders/x/files"),
                PathBuf::from("/var/folders/x/files"),
            )],
            ignored: vec![PathBuf::from("/private/var/folders/x/files/.kura")],
        };
        assert_eq!(
            map.indexed(Path::new("/private/var/folders/x/files/a/b.txt")),
            Some(PathBuf::from("/var/folders/x/files/a/b.txt"))
        );
        assert_eq!(
            map.indexed(Path::new("/var/folders/x/files/c.txt")),
            Some(PathBuf::from("/var/folders/x/files/c.txt"))
        );
        assert_eq!(map.indexed(Path::new("/private/var/folders/x/other")), None);
        assert_eq!(
            map.indexed(Path::new("/private/var/folders/x/files/.kura/index")),
            None
        );
    }
}
