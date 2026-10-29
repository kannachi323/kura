//! Watches the providers folder and reports files that settled after a change.
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use notify::{RecursiveMode, Watcher};

use crate::{names, Kuragine, Result};

/// Editors save in bursts (temp file, rename, metadata); wait for quiet.
const QUIET: Duration = Duration::from_millis(1500);

impl Kuragine {
    /// Runs until the process ends: first catches up on changes made while
    /// Kura was closed, then reports changes as they settle.
    pub async fn watch(&self) -> Result<()> {
        let root = self.providers_root().to_path_buf();
        tokio::fs::create_dir_all(&root).await?;
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<PathBuf>();
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                if let Ok(event) = event {
                    for path in event.paths {
                        let _ = sender.send(path);
                    }
                }
            })
            .map_err(|error| {
                crate::Error::invalid(format!("Can't watch cloud folders: {error}"))
            })?;
        watcher
            .watch(&root, RecursiveMode::Recursive)
            .map_err(|error| {
                crate::Error::invalid(format!("Can't watch cloud folders: {error}"))
            })?;

        self.scan_local_changes().await;

        let mut pending: HashMap<PathBuf, Instant> = HashMap::new();
        let mut tick = tokio::time::interval(Duration::from_millis(300));
        loop {
            tokio::select! {
                path = receiver.recv() => {
                    let Some(path) = path else { return Ok(()) };
                    if is_relevant(&root, &path) {
                        pending.insert(path, Instant::now());
                    }
                }
                _ = tick.tick() => {
                    let due: Vec<PathBuf> = pending
                        .iter()
                        .filter(|(_, changed)| changed.elapsed() >= QUIET)
                        .map(|(path, _)| path.clone())
                        .collect();
                    for path in due {
                        pending.remove(&path);
                        self.local_change(&path).await;
                    }
                }
            }
        }
    }
}

/// Skips Kuragine's own folders (`.partial`) and editor scratch files.
fn is_relevant(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    let Some(first) = relative.components().next() else {
        return false;
    };
    if first.as_os_str().to_string_lossy().starts_with('.') {
        return false;
    }
    !path
        .file_name()
        .map(|name| names::is_scratch_name(&name.to_string_lossy()))
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_internal_folders_and_scratch_files() {
        let root = Path::new("/p");
        assert!(is_relevant(root, Path::new("/p/work/a.txt")));
        assert!(!is_relevant(root, Path::new("/p/.partial/x")));
        assert!(!is_relevant(root, Path::new("/p/work/~$a.docx")));
        assert!(!is_relevant(root, Path::new("/elsewhere/a.txt")));
        assert!(!is_relevant(root, Path::new("/p")));
    }
}
