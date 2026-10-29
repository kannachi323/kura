//! Remembers the extra Files windows (New Window) and the folder each one
//! shows, so they reopen where they were after Kura restarts. Closing a
//! window forgets it; quitting Kura keeps the list.

use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct ExtraWindowsService {
    path: PathBuf,
    state: Arc<Mutex<State>>,
}

#[derive(Default)]
struct State {
    /// Window label → the folder it shows ("" until it reports one).
    windows: BTreeMap<String, String>,
    /// Set once Kura is quitting: windows closed by the quit are kept.
    exiting: bool,
}

impl ExtraWindowsService {
    pub fn new(path: PathBuf) -> Self {
        let windows = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            path,
            state: Arc::new(Mutex::new(State {
                windows,
                exiting: false,
            })),
        }
    }

    /// The windows to reopen at launch: (label, folder).
    pub fn saved(&self) -> Vec<(String, String)> {
        self.with_state(|state| {
            state
                .windows
                .iter()
                .map(|(label, folder)| (label.clone(), folder.clone()))
                .collect()
        })
    }

    pub fn opened(&self, label: &str, folder: &str) {
        self.update(|state| {
            state.windows.insert(label.to_owned(), folder.to_owned());
        });
    }

    /// Records the folder a window now shows. Ignores the main window.
    pub fn set_folder(&self, label: &str, folder: &str) {
        self.update(|state| {
            if let Some(current) = state.windows.get_mut(label) {
                *current = folder.to_owned();
            }
        });
    }

    pub fn closed(&self, label: &str) {
        self.update(|state| {
            if !state.exiting {
                state.windows.remove(label);
            }
        });
    }

    pub fn exiting(&self) {
        self.with_state(|state| state.exiting = true);
    }

    fn with_state<T>(&self, f: impl FnOnce(&mut State) -> T) -> T {
        let mut guard = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&mut guard)
    }

    fn update(&self, f: impl FnOnce(&mut State)) {
        let windows = self.with_state(|state| {
            f(state);
            state.windows.clone()
        });
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(bytes) = serde_json::to_vec(&windows) {
            let temporary = self.path.with_extension("json.tmp");
            if std::fs::write(&temporary, bytes).is_ok() {
                let _ = std::fs::rename(&temporary, &self.path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_forgets_a_window_but_quitting_keeps_them() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("windows.json");
        let windows = ExtraWindowsService::new(path.clone());
        windows.opened("files-a", "/a");
        windows.opened("files-b", "/b");
        windows.set_folder("files-b", "/b/c");
        windows.set_folder("main", "/ignored");
        windows.closed("files-a");
        windows.exiting();
        windows.closed("files-b");

        let reopened = ExtraWindowsService::new(path);
        assert_eq!(
            reopened.saved(),
            vec![("files-b".to_owned(), "/b/c".to_owned())]
        );
    }
}
