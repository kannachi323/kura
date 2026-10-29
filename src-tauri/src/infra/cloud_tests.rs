//! Cloud folders through the explorer and the operation queue, against
//! Kuragine's in-memory drive.
use std::{path::PathBuf, sync::Arc, time::Duration};

use kuragine::cloud::{fake::FakeDrive, ROOT_ID};

use crate::domain::explorer::{
    ClipboardOperation, ExplorerLocationKind, ListDirectoryRequest, PasteItem, PasteItemsRequest,
    PrepareOpenItemRequest, RenameItemRequest,
};
use crate::domain::operation_queue::{ConflictPolicy, OperationKind, OperationStatus};
use crate::infra::{
    environment::AppEnvironmentService,
    explorer::ExplorerService,
    explorer_library::ExplorerLibraryService,
    operation_queue::OperationQueueService,
    transfers::{TransferFilter, TransferService},
};

struct Fixture {
    explorer: ExplorerService,
    queue: OperationQueueService,
    transfers: TransferService,
    drive: Arc<FakeDrive>,
    home: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let home = std::env::temp_dir().join(format!(
            "kura-cloud-{label}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let environment = AppEnvironmentService::for_test_home(home.clone());
        std::fs::create_dir_all(environment.db_dir()).unwrap();
        let transfers = TransferService::new(environment.clone());
        let library = ExplorerLibraryService::new(environment.clone());
        let explorer = ExplorerService::new(environment, transfers.clone(), library);
        let drive = explorer.cloud().connect_fake("drive");
        let queue = OperationQueueService::new(explorer.clone(), transfers.clone());
        Self {
            explorer,
            queue,
            transfers,
            drive,
            home,
        }
    }

    fn path(&self, rel: &str) -> String {
        self.explorer
            .cloud()
            .local("drive", rel)
            .to_string_lossy()
            .into_owned()
    }

    async fn list(&self, path: String) -> crate::domain::explorer::DirectoryListing {
        self.explorer
            .list_directory(ListDirectoryRequest {
                path: Some(path),
                show_hidden: Some(false),
                force_remote_refresh: None,
            })
            .await
            .unwrap()
    }

    async fn open(&self, rel: &str) -> PathBuf {
        let prepared = self
            .explorer
            .prepare_open_item(PrepareOpenItemRequest {
                path: self.path(rel),
                size_bytes: None,
                remote_modified: None,
            })
            .await
            .unwrap();
        PathBuf::from(prepared.local_path)
    }

    async fn newest_operation(&self) -> u64 {
        self.queue
            .snapshot()
            .await
            .operations
            .iter()
            .map(|operation| operation.operation_id)
            .max()
            .unwrap()
    }

    async fn wait_for(&self, operation_id: u64, expected: OperationStatus) {
        let mut last = None;
        for _ in 0..250 {
            let snapshot = self.queue.snapshot().await;
            let operation = snapshot
                .operations
                .iter()
                .find(|operation| operation.operation_id == operation_id)
                .cloned();
            if operation.as_ref().map(|operation| operation.status) == Some(expected) {
                return;
            }
            last = operation;
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("operation {operation_id} did not reach {expected:?}: {last:?}");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.home);
    }
}

fn edit(path: &std::path::Path, content: &[u8]) {
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
async fn cloud_folders_list_and_open_like_local_ones() {
    let fx = Fixture::new("list");
    fx.drive.add_file(ROOT_ID, "a.txt", b"hello");

    let providers = fx
        .explorer
        .cloud()
        .providers_root()
        .to_string_lossy()
        .into_owned();
    let accounts = fx.list(providers).await;
    assert_eq!(accounts.entries.len(), 1);
    assert_eq!(accounts.entries[0].name, "drive");

    let listing = fx.list(fx.path("/")).await;
    let entry = &listing.entries[0];
    assert_eq!(entry.name, "a.txt");
    assert!(matches!(
        entry.location.kind,
        ExplorerLocationKind::RemoteProvider
    ));
    assert_eq!(entry.location.cloud_state.as_deref(), Some("online"));

    let local = fx.open("/a.txt").await;
    assert_eq!(std::fs::read(&local).unwrap(), b"hello");
    let listing = fx.list(fx.path("/")).await;
    assert_eq!(
        listing.entries[0].location.cloud_state.as_deref(),
        Some("downloaded")
    );
    // Opening recorded a download in Transfers.
    let history = fx
        .transfers
        .snapshot(TransferFilter::default())
        .await
        .unwrap();
    assert!(history.rows.iter().any(|row| row.file_name == "a.txt"));
}

#[tokio::test]
async fn cloud_renames_run_in_the_queue_and_undo() {
    let fx = Fixture::new("rename");
    let id = fx.drive.add_file(ROOT_ID, "a.txt", b"x");
    fx.list(fx.path("/")).await;

    fx.queue
        .enqueue_rename_item(RenameItemRequest {
            path: fx.path("/a.txt"),
            new_name: "b.txt".into(),
            source_is_directory: Some(false),
        })
        .await
        .unwrap();
    let operation = fx.newest_operation().await;
    fx.wait_for(operation, OperationStatus::Completed).await;
    assert_eq!(
        fx.drive.find(ROOT_ID, "b.txt").as_deref(),
        Some(id.as_str())
    );

    let history = fx
        .transfers
        .snapshot(TransferFilter::default())
        .await
        .unwrap();
    let row = history
        .rows
        .iter()
        .find(|row| row.file_name == "b.txt" && row.undoable)
        .expect("the cloud rename can be undone");
    assert_eq!(row.remote_source_name, "drive");
    fx.queue.undo(row.undo_token_id).await.unwrap();
    let undo = fx.newest_operation().await;
    fx.wait_for(undo, OperationStatus::Completed).await;
    assert_eq!(
        fx.drive.find(ROOT_ID, "a.txt").as_deref(),
        Some(id.as_str())
    );
}

#[tokio::test]
async fn saved_edits_upload_and_conflicts_wait_for_a_decision() {
    let fx = Fixture::new("upload");
    let id = fx.drive.add_file(ROOT_ID, "a.txt", b"one");
    let local = fx.open("/a.txt").await;

    edit(&local, b"two");
    fx.queue
        .enqueue_cloud_upload(fx.path("/a.txt"))
        .await
        .unwrap();
    let upload = fx.newest_operation().await;
    let kind = fx
        .queue
        .snapshot()
        .await
        .operations
        .iter()
        .find(|operation| operation.operation_id == upload)
        .unwrap()
        .kind;
    assert_eq!(kind, OperationKind::Upload);
    fx.wait_for(upload, OperationStatus::Completed).await;
    assert_eq!(fx.drive.content(&id), b"two");

    // Changed on both sides: the upload stops and offers both versions.
    fx.drive.set_content(&id, b"theirs");
    edit(&local, b"mine");
    fx.queue
        .enqueue_cloud_upload(fx.path("/a.txt"))
        .await
        .unwrap();
    let conflicted = fx.newest_operation().await;
    fx.wait_for(conflicted, OperationStatus::WaitingForResolution)
        .await;
    let dialog = fx.queue.snapshot().await.conflict_dialog;
    assert!(dialog.open);
    assert_eq!(dialog.compare_local_path, local.to_string_lossy());
    assert_eq!(
        std::fs::read(&dialog.compare_cloud_path).unwrap(),
        b"theirs"
    );
    assert_eq!(fx.drive.content(&id), b"theirs");

    fx.queue
        .resolve_conflict(conflicted, ConflictPolicy::Replace, false)
        .await
        .unwrap();
    fx.wait_for(conflicted, OperationStatus::Completed).await;
    assert_eq!(fx.drive.content(&id), b"mine");
}

#[tokio::test]
async fn pasting_uploads_into_and_downloads_out_of_the_cloud() {
    let fx = Fixture::new("paste");
    let outside = fx.home.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    let report = outside.join("report.txt");
    std::fs::write(&report, b"local").unwrap();
    fx.list(fx.path("/")).await;

    fx.queue
        .enqueue_paste_items(PasteItemsRequest {
            sources: vec![PasteItem {
                path: report.to_string_lossy().into_owned(),
                is_directory: false,
                size_bytes: Some(5),
                remote_modified: None,
            }],
            destination_directory: fx.path("/"),
            operation: ClipboardOperation::Copy,
            target_name: None,
        })
        .await
        .unwrap();
    let upload = fx.newest_operation().await;
    fx.wait_for(upload, OperationStatus::Completed).await;
    let uploaded = fx.drive.find(ROOT_ID, "report.txt").unwrap();
    assert_eq!(fx.drive.content(&uploaded), b"local");

    fx.drive.add_file(ROOT_ID, "cloud.txt", b"from the cloud");
    fx.list(fx.path("/")).await;
    fx.explorer.cloud().refresh("drive", "/").await.unwrap();
    fx.queue
        .enqueue_paste_items(PasteItemsRequest {
            sources: vec![PasteItem {
                path: fx.path("/cloud.txt"),
                is_directory: false,
                size_bytes: None,
                remote_modified: None,
            }],
            destination_directory: outside.to_string_lossy().into_owned(),
            operation: ClipboardOperation::Copy,
            target_name: None,
        })
        .await
        .unwrap();
    let download = fx.newest_operation().await;
    fx.wait_for(download, OperationStatus::Completed).await;
    assert_eq!(
        std::fs::read(outside.join("cloud.txt")).unwrap(),
        b"from the cloud"
    );
}
