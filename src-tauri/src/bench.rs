//! Entry points for `examples/bench.rs`. Not used by the app.

use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use crate::{
    domain::explorer::{self, ListDirectoryRequest},
    infra::{
        environment::AppEnvironmentService,
        search::SearchService,
        search::{SearchQueryRequest, SearchQueryScope, SearchRuleMatchMode, SearchScanRequest},
    },
};

/// Timings for one listing of `path`, split by phase.
pub struct ListingTiming {
    pub entries: usize,
    /// read_dir + per-entry metadata + sort (the Rust listing pipeline).
    pub list: Duration,
    /// serde_json encode of the result, which is what Tauri IPC sends.
    pub serialize: Duration,
    pub json_bytes: usize,
}

pub fn list_directory(path: &Path) -> Result<ListingTiming, String> {
    let started = Instant::now();
    let listing = explorer::list_directory(
        PathBuf::new(),
        ListDirectoryRequest {
            path: Some(path.display().to_string()),
            show_hidden: Some(false),
            force_remote_refresh: None,
        },
    )
    .map_err(|error| error.to_string())?;
    let list = started.elapsed();
    let started = Instant::now();
    let json = serde_json::to_vec(&listing).map_err(|error| error.to_string())?;
    Ok(ListingTiming {
        entries: listing.entries.len(),
        list,
        serialize: started.elapsed(),
        json_bytes: json.len(),
    })
}

/// Writes the listing JSON so the frontend parse cost can be timed too.
pub fn write_listing_json(path: &Path, out: &Path) -> Result<(), String> {
    let listing = explorer::list_directory(
        PathBuf::new(),
        ListDirectoryRequest {
            path: Some(path.display().to_string()),
            show_hidden: Some(false),
            force_remote_refresh: None,
        },
    )
    .map_err(|error| error.to_string())?;
    let json = serde_json::to_vec(&listing).map_err(|error| error.to_string())?;
    std::fs::write(out, json).map_err(|error| error.to_string())
}

/// What one scan did, from the search status after it finished.
pub struct ScanCounts {
    pub indexed: u64,
    pub added: u64,
    pub updated: u64,
    pub removed: u64,
    pub unchanged: u64,
}

/// One batch of file-system events the index watcher applied.
pub struct WatchUpdate {
    pub added: u64,
    pub updated: u64,
    pub removed: u64,
    pub published: bool,
    /// When the watcher received the batch's first event.
    pub first_event: Instant,
    /// When the batch went quiet and its update began.
    pub applying: Instant,
    /// When the update was committed and published.
    pub committed: Instant,
}

/// A search index kept under `data_root/.kura`, separate from the user's.
pub struct SearchBench {
    service: SearchService,
}

impl SearchBench {
    pub fn new(data_root: PathBuf) -> Self {
        Self {
            service: SearchService::new(AppEnvironmentService::new_with_data_root(Some(data_root))),
        }
    }

    /// Indexes `root` and waits for the scan to finish. Returns the item count.
    pub async fn scan(&self, root: &Path) -> Result<u64, String> {
        self.scan_with(root, false)
            .await
            .map(|counts| counts.indexed)
    }

    /// Scans `root` (incrementally when asked) and returns what the scan changed.
    pub async fn scan_with(&self, root: &Path, incremental: bool) -> Result<ScanCounts, String> {
        self.service
            .start_scan(scan_request(root, incremental, false))
            .await
            .map_err(|error| error.to_string())?;
        loop {
            tokio::time::sleep(Duration::from_millis(25)).await;
            let status = self
                .service
                .status()
                .await
                .map_err(|error| error.to_string())?;
            if !status.scan_in_progress {
                if let Some(error) = status.last_scan_error {
                    return Err(error);
                }
                return Ok(ScanCounts {
                    indexed: status.indexed_item_count,
                    added: status.last_scan_added_item_count,
                    updated: status.last_scan_updated_item_count,
                    removed: status.last_scan_removed_item_count,
                    unchanged: status.last_scan_unchanged_item_count,
                });
            }
        }
    }

    /// Keeps the index of `root` current from file-system events, as the app
    /// does between scans. `on_update` receives every batch applied.
    pub fn watch(
        &self,
        root: &Path,
        on_update: impl Fn(WatchUpdate) + Send + Sync + 'static,
    ) -> Result<(), String> {
        self.service.on_watch_update(move |report| {
            on_update(WatchUpdate {
                added: report.changes.added,
                updated: report.changes.updated,
                removed: report.changes.removed,
                published: report.changes.published,
                first_event: report.first_event,
                applying: report.applying,
                committed: report.committed,
            })
        });
        self.service
            .watch(&scan_request(root, true, true))
            .map_err(|error| error.to_string())
    }

    /// Runs one query and returns the number of results.
    pub async fn query(&self, text: &str) -> Result<usize, String> {
        self.query_paths(text).await.map(|paths| paths.len())
    }

    /// Runs one query and returns the paths it found.
    pub async fn query_paths(&self, text: &str) -> Result<Vec<String>, String> {
        self.service
            .query(SearchQueryRequest {
                query: text.to_owned(),
                current_path: None,
                scope: SearchQueryScope::Everything,
                include_files: true,
                include_directories: true,
                include_hidden: false,
                limit: None,
                rules: Vec::new(),
                match_mode: SearchRuleMatchMode::default(),
            })
            .await
            .map(|results| {
                results
                    .into_iter()
                    .map(|result| result.entry.path)
                    .collect()
            })
            .map_err(|error| error.to_string())
    }
}

fn scan_request(root: &Path, incremental: bool, watch: bool) -> SearchScanRequest {
    SearchScanRequest {
        roots: vec![root.display().to_string()],
        include_local: true,
        include_remotes: false,
        remote_names: Vec::new(),
        max_depth: None,
        ignored_paths: Vec::new(),
        incremental,
        watch,
    }
}
