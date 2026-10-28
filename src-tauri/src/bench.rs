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
        self.service
            .start_scan(SearchScanRequest {
                roots: vec![root.display().to_string()],
                include_local: true,
                include_remotes: false,
                remote_names: Vec::new(),
                max_depth: None,
                ignored_paths: Vec::new(),
                incremental: false,
            })
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
                return Ok(status.indexed_item_count);
            }
        }
    }

    /// Runs one query and returns the number of results.
    pub async fn query(&self, text: &str) -> Result<usize, String> {
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
            .map(|results| results.len())
            .map_err(|error| error.to_string())
    }
}
