use std::{path::Path, sync::Arc, time::Duration};

use tempfile::TempDir;

use super::*;
use crate::cloud::{fake::FakeDrive, secrets::MemoryStore, NoProgress, ROOT_ID};

struct Fixture {
    engine: Kuragine,
    drive: Arc<FakeDrive>,
    dir: TempDir,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let config = KuragineConfig {
            providers_root: dir.path().join("providers"),
            conflicts_dir: dir.path().join("conflicts"),
            db_path: dir.path().join("kuragine.db"),
            secrets: Arc::new(MemoryStore::default()),
        };
        let engine = Kuragine::with_store(config, Store::in_memory().unwrap());
        engine
            .store()
            .put_remote(&RemoteRow {
                name: "drive".into(),
                kind: ProviderKind::GoogleDrive,
                email: "me@example.com".into(),
                display_name: "Me".into(),
                drive_type: String::new(),
                created_ms: 0,
            })
            .unwrap();
        let drive = Arc::new(FakeDrive::new());
        engine.set_provider("drive", Provider::fake(drive.clone()));
        Self { engine, drive, dir }
    }

    fn local(&self, rel: &str) -> PathBuf {
        self.engine.local("drive", rel)
    }

    async fn state(&self, rel: &str) -> CopyState {
        self.engine
            .entry("drive", rel)
            .await
            .unwrap()
            .unwrap()
            .state
    }
}

/// Writes new content with a later modified time, as an editor would.
fn edit(path: &Path, content: &[u8]) {
    std::fs::write(path, content).unwrap();
    let later = std::time::SystemTime::now() + Duration::from_secs(5);
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(later)
        .unwrap();
}

#[tokio::test]
async fn lists_with_unique_local_names() {
    let fx = Fixture::new();
    fx.drive.add_file(ROOT_ID, "a.txt", b"1");
    fx.drive.add_file(ROOT_ID, "a.txt", b"2");
    fx.drive.add_folder(ROOT_ID, "Docs");
    let mut names: Vec<String> = fx
        .engine
        .list("drive", "/")
        .await
        .unwrap()
        .into_iter()
        .map(|entry| entry.name)
        .collect();
    names.sort();
    assert_eq!(names, vec!["Docs", "a (2).txt", "a.txt"]);
}

#[tokio::test]
async fn resolves_paths_under_the_providers_folder() {
    let fx = Fixture::new();
    let root = fx.engine.providers_root().to_path_buf();
    let resolved = fx
        .engine
        .resolve(&root.join("Drive").join("a").join("b.txt"))
        .unwrap();
    assert_eq!(resolved.remote, "drive");
    assert_eq!(resolved.rel, "/a/b.txt");
    assert!(fx
        .engine
        .resolve(&root.join(".partial").join("x"))
        .is_none());
    assert!(fx.engine.resolve(&root.join("unknown").join("x")).is_none());
    assert!(fx.engine.resolve(fx.dir.path()).is_none());
}

#[tokio::test]
async fn opening_downloads_once_and_records_the_base() {
    let fx = Fixture::new();
    let docs = fx.drive.add_folder(ROOT_ID, "Docs");
    fx.drive.add_file(&docs, "note.txt", b"hello");
    // Resolving a nested path lists the folders on the way.
    let path = fx
        .engine
        .ensure_local("drive", "/Docs/note.txt", &NoProgress)
        .await
        .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"hello");
    assert_eq!(fx.state("/Docs/note.txt").await, CopyState::Downloaded);
    // The local file carries the cloud's modified time.
    let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
    assert_eq!(
        modified
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
        1000
    );
}

#[tokio::test]
async fn local_edits_upload_and_become_the_new_base() {
    let fx = Fixture::new();
    let id = fx.drive.add_file(ROOT_ID, "a.txt", b"one");
    let path = fx
        .engine
        .ensure_local("drive", "/a.txt", &NoProgress)
        .await
        .unwrap();
    let mut events = fx.engine.subscribe();

    edit(&path, b"two");
    assert_eq!(fx.state("/a.txt").await, CopyState::Modified);
    fx.engine.local_change(&path).await;
    match events.try_recv().unwrap() {
        EngineEvent::UploadNeeded { rel, .. } => assert_eq!(rel, "/a.txt"),
        other => panic!("unexpected {other:?}"),
    }
    let outcome = fx
        .engine
        .upload_copy("drive", "/a.txt", &NoProgress)
        .await
        .unwrap();
    assert!(matches!(outcome, UploadOutcome::Uploaded));
    assert_eq!(fx.drive.content(&id), b"two");
    assert_eq!(fx.state("/a.txt").await, CopyState::Downloaded);

    // Uploading again finds nothing to do.
    let outcome = fx
        .engine
        .upload_copy("drive", "/a.txt", &NoProgress)
        .await
        .unwrap();
    assert!(matches!(outcome, UploadOutcome::Unchanged));
}

#[tokio::test]
async fn touching_without_changing_content_asks_for_nothing() {
    let fx = Fixture::new();
    fx.drive.add_file(ROOT_ID, "a.txt", b"same");
    let path = fx
        .engine
        .ensure_local("drive", "/a.txt", &NoProgress)
        .await
        .unwrap();
    let mut events = fx.engine.subscribe();
    edit(&path, b"same");
    fx.engine.local_change(&path).await;
    assert!(events.try_recv().is_err());
    assert_eq!(fx.state("/a.txt").await, CopyState::Downloaded);
}

#[tokio::test]
async fn unchanged_copies_follow_the_cloud() {
    let fx = Fixture::new();
    let id = fx.drive.add_file(ROOT_ID, "a.txt", b"old");
    let path = fx
        .engine
        .ensure_local("drive", "/a.txt", &NoProgress)
        .await
        .unwrap();
    fx.drive.set_content(&id, b"new");
    let mut events = fx.engine.subscribe();
    fx.engine.check_freshness("drive", "/a.txt").await.unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"new");
    assert!(matches!(
        events.try_recv().unwrap(),
        EngineEvent::CopyUpdated { .. }
    ));
}

#[tokio::test]
async fn freshness_never_replaces_a_local_edit() {
    let fx = Fixture::new();
    let id = fx.drive.add_file(ROOT_ID, "a.txt", b"old");
    let path = fx
        .engine
        .ensure_local("drive", "/a.txt", &NoProgress)
        .await
        .unwrap();
    edit(&path, b"mine");
    fx.drive.set_content(&id, b"theirs");
    fx.engine.check_freshness("drive", "/a.txt").await.unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"mine");
}

async fn conflicted(fx: &Fixture) -> (String, PathBuf, ConflictInfo) {
    let id = fx.drive.add_file(ROOT_ID, "a.txt", b"base");
    let path = fx
        .engine
        .ensure_local("drive", "/a.txt", &NoProgress)
        .await
        .unwrap();
    edit(&path, b"mine");
    fx.drive.set_content(&id, b"theirs");
    let outcome = fx
        .engine
        .upload_copy("drive", "/a.txt", &NoProgress)
        .await
        .unwrap();
    let UploadOutcome::Conflict(info) = outcome else {
        panic!("expected a conflict, got {outcome:?}");
    };
    (id, path, info)
}

#[tokio::test]
async fn uploads_stop_when_the_cloud_changed() {
    let fx = Fixture::new();
    let (id, path, info) = conflicted(&fx).await;
    assert_eq!(fx.drive.content(&id), b"theirs");
    assert_eq!(std::fs::read(&path).unwrap(), b"mine");
    assert_eq!(std::fs::read(&info.remote_copy_path).unwrap(), b"theirs");
    assert_eq!(fx.state("/a.txt").await, CopyState::Conflict);
    assert_eq!(fx.engine.conflicts().unwrap().len(), 1);
    // Further uploads wait for the decision.
    let again = fx
        .engine
        .upload_copy("drive", "/a.txt", &NoProgress)
        .await
        .unwrap();
    assert!(matches!(again, UploadOutcome::Conflict(_)));
}

#[tokio::test]
async fn keep_mine_overwrites_the_cloud() {
    let fx = Fixture::new();
    let (id, _, info) = conflicted(&fx).await;
    let outcome = fx
        .engine
        .resolve_conflict("drive", "/a.txt", ConflictChoice::KeepMine, &NoProgress)
        .await
        .unwrap();
    assert!(matches!(outcome, UploadOutcome::Uploaded));
    assert_eq!(fx.drive.content(&id), b"mine");
    assert!(fx.engine.conflicts().unwrap().is_empty());
    assert!(!info.remote_copy_path.exists());
    assert_eq!(fx.state("/a.txt").await, CopyState::Downloaded);
}

#[tokio::test]
async fn keep_theirs_sets_the_local_edit_aside() {
    let fx = Fixture::new();
    let (id, path, _) = conflicted(&fx).await;
    fx.engine
        .resolve_conflict("drive", "/a.txt", ConflictChoice::KeepTheirs, &NoProgress)
        .await
        .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"theirs");
    assert_eq!(fx.drive.content(&id), b"theirs");
    let kept: Vec<_> = std::fs::read_dir(fx.dir.path().join("conflicts/drive/kept"))
        .unwrap()
        .flatten()
        .collect();
    assert_eq!(kept.len(), 1);
    assert_eq!(std::fs::read(kept[0].path()).unwrap(), b"mine");
    assert_eq!(fx.state("/a.txt").await, CopyState::Downloaded);
}

#[tokio::test]
async fn keep_both_uploads_the_edit_under_a_new_name() {
    let fx = Fixture::new();
    let (id, path, _) = conflicted(&fx).await;
    fx.engine
        .resolve_conflict("drive", "/a.txt", ConflictChoice::KeepBoth, &NoProgress)
        .await
        .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"theirs");
    assert_eq!(fx.drive.content(&id), b"theirs");
    let entries = fx.engine.list("drive", "/").await.unwrap();
    let copy = entries
        .iter()
        .find(|entry| entry.name.starts_with("a (conflict "))
        .expect("conflict copy listed");
    let copy_id = fx.drive.find(ROOT_ID, &copy.name).unwrap();
    assert_eq!(fx.drive.content(&copy_id), b"mine");
}

#[tokio::test]
async fn new_local_files_upload_into_new_cloud_folders() {
    let fx = Fixture::new();
    fx.engine.list("drive", "/").await.unwrap();
    let path = fx.local("/Projects/plan.md");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"plan").unwrap();
    let outcome = fx
        .engine
        .upload_copy("drive", "/Projects/plan.md", &NoProgress)
        .await
        .unwrap();
    assert!(matches!(outcome, UploadOutcome::Uploaded));
    let folder = fx.drive.find(ROOT_ID, "Projects").unwrap();
    let file = fx.drive.find(&folder, "plan.md").unwrap();
    assert_eq!(fx.drive.content(&file), b"plan");
}

#[tokio::test]
async fn a_file_matching_the_cloud_is_adopted_not_conflicted() {
    let fx = Fixture::new();
    fx.drive.add_file(ROOT_ID, "a.txt", b"same");
    fx.engine.list("drive", "/").await.unwrap();
    let path = fx.local("/a.txt");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"same").unwrap();
    let outcome = fx
        .engine
        .upload_copy("drive", "/a.txt", &NoProgress)
        .await
        .unwrap();
    assert!(matches!(outcome, UploadOutcome::Unchanged));
    assert_eq!(fx.state("/a.txt").await, CopyState::Downloaded);
}

#[tokio::test]
async fn renaming_and_moving_carry_the_local_copy() {
    let fx = Fixture::new();
    let id = fx.drive.add_file(ROOT_ID, "a.txt", b"x");
    fx.drive.add_folder(ROOT_ID, "Docs");
    let old = fx
        .engine
        .ensure_local("drive", "/a.txt", &NoProgress)
        .await
        .unwrap();

    let renamed = fx.engine.rename("drive", "/a.txt", "b.txt").await.unwrap();
    assert_eq!(renamed, "/b.txt");
    assert!(!old.exists());
    assert_eq!(fx.state("/b.txt").await, CopyState::Downloaded);

    let moved = fx
        .engine
        .move_to("drive", "/b.txt", "/Docs", None)
        .await
        .unwrap();
    assert_eq!(moved, "/Docs/b.txt");
    assert_eq!(std::fs::read(fx.local("/Docs/b.txt")).unwrap(), b"x");
    assert_eq!(fx.state("/Docs/b.txt").await, CopyState::Downloaded);
    let docs = fx.drive.find(ROOT_ID, "Docs").unwrap();
    assert_eq!(fx.drive.find(&docs, "b.txt").as_deref(), Some(id.as_str()));
}

#[tokio::test]
async fn renames_in_the_cloud_move_the_local_copy() {
    let fx = Fixture::new();
    let id = fx.drive.add_file(ROOT_ID, "a.txt", b"x");
    fx.engine
        .ensure_local("drive", "/a.txt", &NoProgress)
        .await
        .unwrap();
    let item = fx
        .drive
        .stat(&crate::cloud::RemoteItem {
            id: id.clone(),
            ..Default::default()
        })
        .await
        .unwrap();
    fx.drive
        .rename_move(&item, "renamed.txt", None)
        .await
        .unwrap();
    fx.engine.refresh("drive", "/").await.unwrap();
    assert!(fx.local("/renamed.txt").exists());
    assert!(!fx.local("/a.txt").exists());
    assert_eq!(fx.state("/renamed.txt").await, CopyState::Downloaded);
}

#[tokio::test]
async fn cloud_deletions_remove_only_unchanged_copies() {
    let fx = Fixture::new();
    let keep = fx.drive.add_file(ROOT_ID, "edited.txt", b"1");
    let drop = fx.drive.add_file(ROOT_ID, "plain.txt", b"1");
    let edited = fx
        .engine
        .ensure_local("drive", "/edited.txt", &NoProgress)
        .await
        .unwrap();
    let plain = fx
        .engine
        .ensure_local("drive", "/plain.txt", &NoProgress)
        .await
        .unwrap();
    edit(&edited, b"local work");
    for id in [&keep, &drop] {
        let item = fx
            .drive
            .stat(&crate::cloud::RemoteItem {
                id: id.to_string(),
                ..Default::default()
            })
            .await
            .unwrap();
        fx.drive.trash(&item).await.unwrap();
    }
    fx.engine.refresh("drive", "/").await.unwrap();
    assert!(!plain.exists());
    assert_eq!(std::fs::read(&edited).unwrap(), b"local work");
}

#[tokio::test]
async fn trashing_sets_unsaved_edits_aside() {
    let fx = Fixture::new();
    let docs = fx.drive.add_folder(ROOT_ID, "Docs");
    let a = fx.drive.add_file(&docs, "a.txt", b"1");
    fx.drive.add_file(&docs, "b.txt", b"1");
    let a_path = fx
        .engine
        .ensure_local("drive", "/Docs/a.txt", &NoProgress)
        .await
        .unwrap();
    fx.engine
        .ensure_local("drive", "/Docs/b.txt", &NoProgress)
        .await
        .unwrap();
    edit(&a_path, b"unsaved");
    fx.engine.trash("drive", "/Docs").await.unwrap();
    assert!(fx.drive.is_trashed(&docs));
    assert!(!fx.local("/Docs").exists());
    let kept: Vec<_> = std::fs::read_dir(fx.dir.path().join("conflicts/drive/kept"))
        .unwrap()
        .flatten()
        .collect();
    assert_eq!(kept.len(), 1);
    assert_eq!(std::fs::read(kept[0].path()).unwrap(), b"unsaved");
    let _ = a;
}

#[tokio::test]
async fn copies_folders_and_transfers_to_and_from_disk() {
    let fx = Fixture::new();
    let docs = fx.drive.add_folder(ROOT_ID, "Docs");
    fx.drive.add_file(&docs, "a.txt", b"aa");
    fx.engine.list("drive", "/").await.unwrap();

    let copied = fx
        .engine
        .copy_within("drive", "/Docs", "/", Some("Docs copy"), &NoProgress)
        .await
        .unwrap();
    assert_eq!(copied, "/Docs copy");
    let copy = fx.drive.find(ROOT_ID, "Docs copy").unwrap();
    assert!(fx.drive.find(&copy, "a.txt").is_some());

    let out = fx.dir.path().join("out");
    fx.engine
        .download_to("drive", "/Docs", &out, &NoProgress)
        .await
        .unwrap();
    assert_eq!(std::fs::read(out.join("a.txt")).unwrap(), b"aa");

    std::fs::write(out.join("b.txt"), b"bb").unwrap();
    fx.engine
        .upload_path(&out, "drive", "/", Some("Uploaded"), &NoProgress)
        .await
        .unwrap();
    let uploaded = fx.drive.find(ROOT_ID, "Uploaded").unwrap();
    let b = fx.drive.find(&uploaded, "b.txt").unwrap();
    assert_eq!(fx.drive.content(&b), b"bb");
}

#[tokio::test]
async fn startup_scan_finds_edits_made_while_closed() {
    let fx = Fixture::new();
    fx.drive.add_file(ROOT_ID, "a.txt", b"1");
    let path = fx
        .engine
        .ensure_local("drive", "/a.txt", &NoProgress)
        .await
        .unwrap();
    edit(&path, b"2");
    std::fs::write(fx.local("/new.txt"), b"n").unwrap();
    std::fs::write(fx.local("/~$scratch.docx"), b"x").unwrap();
    let mut events = fx.engine.subscribe();
    fx.engine.scan_local_changes().await;
    let mut seen = Vec::new();
    while let Ok(EngineEvent::UploadNeeded { rel, .. }) = events.try_recv() {
        seen.push(rel);
    }
    seen.sort();
    assert_eq!(seen, vec!["/a.txt", "/new.txt"]);
}

#[tokio::test]
async fn validates_remote_names() {
    assert!(validate_remote_name("Work Drive").is_ok());
    assert!(validate_remote_name(".hidden").is_err());
    assert!(validate_remote_name("a/b").is_err());
    assert!(validate_remote_name("").is_err());
}
