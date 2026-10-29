use std::{
    collections::HashSet,
    env,
    path::Path,
    process::Command,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::app::runtime::KuraRuntime;
use crate::domain::clipboard::{ClipboardImage, ClipboardPayload, ClipboardPayloadKind};
use crate::domain::explorer::{
    ClipboardOperation, CreateItemRequest, DeleteItemsRequest, DirectoryListing,
    ExplorerOperationResult, ExplorerPreviewPayload, GeneratedImageThumbnail, ListDirectoryRequest,
    PasteBlobRequest, PasteItem, PasteItemsRequest, PasteTextRequest, PrepareDragItemsRequest,
    PrepareOpenItemRequest, PreparedDragItem, PreparedDragItemsResult, PreparedDragSkippedItem,
    PreparedOpenItem, RenameItemRequest, RenameItemsRequest,
};
use crate::domain::file_sync::FileSyncPair;
use crate::domain::operation_queue::{ConflictPolicy, OperationQueueSnapshot};
use crate::error::{ApiError, ApiResult};
use crate::infra::devices::{DeviceSnapshot, DeviceUnmountRequest};
use crate::infra::directory_size::{DirectorySizeRecord, DirectorySizeRequest};
use crate::infra::environment::AppEnvironmentSnapshot;
use crate::infra::explorer::SavePreviewRequest;
use crate::infra::explorer_library::{
    ExplorerLibrarySnapshot, RecordLastOpenedRequest, RecordRecentRequest, SetTagsRequest,
};
use crate::infra::file_sync::{FileSyncApplyRequest, FileSyncApplyResult, FileSyncCompareRequest};
use crate::infra::metadata::FileMetadataSnapshot;
use crate::infra::power_pack::{
    ArchiveActionResult, ArchiveCreateRequest, ArchiveExtractRequest, ArchiveListRequest,
    ArchiveListResult, CompareFilesRequest, CompareFilesResult, CompareFoldersRequest,
    CompareFoldersResult, DuplicateScanRequest, DuplicateScanResult, FileToolsActionResult,
    FileToolsChecksumRequest, FileToolsChecksumResult, FileToolsChmodRequest,
    FileToolsReadonlyRequest, FileToolsSymlinkRequest, FileToolsSymlinkTargetRequest,
    FileToolsSymlinkTargetResult, SavedSearch, SavedSearchesSnapshot,
};
use crate::infra::search::{SearchQueryRequest, SearchResult, SearchScanRequest, SearchStatus};
use crate::infra::settings::{OpenWithAssociation, SaveSettingsRequest, SettingsSnapshot};
use crate::infra::transfers::{TransferFilter, TransferPage};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    app_name: &'static str,
    version: &'static str,
    migration_stage: &'static str,
    environment: AppEnvironmentSnapshot,
}

#[derive(Debug, Serialize)]
pub struct ClipboardSnapshot {
    pub local: ClipboardPayload,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardFileBytes {
    pub name: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteAssetStoreRequest {
    pub account_id: String,
    pub space_id: String,
    pub note_id: String,
    pub file_name: String,
    pub mime_type: Option<String>,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteAssetStoreResult {
    pub path: String,
    pub name: String,
    pub mime_type: Option<String>,
    pub byte_size: u64,
}

#[tauri::command]
pub async fn app_snapshot(state: State<'_, KuraRuntime>) -> ApiResult<AppSnapshot> {
    Ok(AppSnapshot {
        app_name: "Kura",
        version: env!("CARGO_PKG_VERSION"),
        migration_stage: "Tauri migration shell",
        environment: state.environment.snapshot(),
    })
}

#[tauri::command]
pub async fn file_metadata_snapshot(
    path: String,
    state: State<'_, KuraRuntime>,
) -> ApiResult<FileMetadataSnapshot> {
    state.metadata.snapshot(path).await
}

#[tauri::command]
pub fn clipboard_snapshot(state: State<'_, KuraRuntime>) -> ClipboardSnapshot {
    ClipboardSnapshot {
        local: state.clipboard.current_local(),
    }
}

#[tauri::command]
pub fn clipboard_set_local(
    payload: ClipboardPayload,
    state: State<'_, KuraRuntime>,
) -> ClipboardPayload {
    state.clipboard.set_local_kura_payload(payload)
}

#[tauri::command]
pub fn clipboard_native_file_refs() -> ApiResult<Vec<PasteItem>> {
    crate::infra::native_clipboard::native_clipboard_file_refs()
}

#[tauri::command]
pub fn clipboard_write_file_refs(items: Vec<PasteItem>) -> ApiResult<bool> {
    crate::infra::native_clipboard::write_native_clipboard_file_refs(&items)
}

#[tauri::command]
pub async fn clipboard_write_file_bytes(items: Vec<ClipboardFileBytes>) -> ApiResult<bool> {
    if items.is_empty() {
        return Err(ApiError::Message(
            "No Library items were selected to copy.".to_owned(),
        ));
    }
    let copy_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let staging = env::temp_dir()
        .join("kura-library-clipboard")
        .join(copy_id.to_string());
    tokio::fs::create_dir_all(&staging).await.map_err(|error| {
        ApiError::Message(format!(
            "The Library clipboard staging folder could not be created: {error}"
        ))
    })?;

    let mut references = Vec::with_capacity(items.len());
    let mut used_names = HashSet::with_capacity(items.len());
    for (index, item) in items.into_iter().enumerate() {
        if item.bytes.is_empty() {
            return Err(ApiError::Message(
                "A selected Library item is empty.".to_owned(),
            ));
        }
        let original_name = Path::new(item.name.trim())
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty() && *name != "." && *name != "..")
            .map(str::to_owned)
            .unwrap_or_else(|| format!("Library item {}", index + 1));
        let original_path = Path::new(&original_name);
        let stem = original_path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("Library item")
            .to_owned();
        let extension = original_path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| format!(".{value}"))
            .unwrap_or_default();
        let mut file_name = original_name;
        let mut suffix = 2;
        while !used_names.insert(file_name.clone()) {
            file_name = format!("{stem} {suffix}{extension}");
            suffix += 1;
        }
        let path = staging.join(file_name);
        tokio::fs::write(&path, &item.bytes)
            .await
            .map_err(|error| {
                ApiError::Message(format!(
                    "A Library item could not be prepared for the clipboard: {error}"
                ))
            })?;
        references.push(PasteItem {
            path: path.to_string_lossy().into_owned(),
            is_directory: false,
            size_bytes: Some(item.bytes.len() as i64),
            remote_modified: None,
        });
    }
    crate::infra::native_clipboard::write_native_clipboard_file_refs(&references)
}

#[tauri::command]
pub async fn explorer_list_directory(
    request: ListDirectoryRequest,
    app: AppHandle,
    state: State<'_, KuraRuntime>,
) -> ApiResult<DirectoryListing> {
    state.explorer.list_directory(request).await
}

/// Watches exactly these folders (the ones open in panes) for changes.
#[tauri::command]
pub fn explorer_watch_directories(
    paths: Vec<String>,
    window: tauri::Window,
    state: State<'_, KuraRuntime>,
) -> ApiResult<()> {
    state.directory_watch.set_directories(window.label(), paths)
}

/// Opens another Kura window, at `path` when given. Extra windows are
/// labelled `files-…`; only the main window restores its workspace.
#[tauri::command]
pub async fn app_open_window(app: AppHandle, path: Option<String>) -> ApiResult<()> {
    let label = format!("files-{}", uuid::Uuid::new_v4().simple());
    let mut url = format!("index.html?window={label}");
    if let Some(path) = path.filter(|path| !path.is_empty()) {
        url.push_str("&path=");
        url.push_str(
            &percent_encoding::utf8_percent_encode(&path, percent_encoding::NON_ALPHANUMERIC)
                .to_string(),
        );
    }
    let builder =
        tauri::WebviewWindowBuilder::new(&app, &label, tauri::WebviewUrl::App(url.into()))
            .title("Kura")
            .inner_size(1100.0, 720.0)
            .min_inner_size(640.0, 420.0);
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .traffic_light_position(tauri::LogicalPosition::new(16.0, 21.5));
    builder
        .build()
        .map(|_| ())
        .map_err(|err| ApiError::Message(format!("Could not open a window: {err}")))
}

#[tauri::command]
pub async fn explorer_directory_size_snapshot(
    paths: Vec<String>,
    state: State<'_, KuraRuntime>,
) -> ApiResult<Vec<DirectorySizeRecord>> {
    state.directory_size.snapshot(paths).await
}

#[tauri::command]
pub async fn explorer_calculate_directory_sizes(
    request: DirectorySizeRequest,
    state: State<'_, KuraRuntime>,
) -> ApiResult<Vec<DirectorySizeRecord>> {
    state.directory_size.calculate(request).await
}

#[tauri::command]
pub async fn search_init(state: State<'_, KuraRuntime>) -> ApiResult<SearchStatus> {
    state.search.init().await
}

#[tauri::command]
pub async fn search_get_status(state: State<'_, KuraRuntime>) -> ApiResult<SearchStatus> {
    state.search.status().await
}

#[tauri::command]
pub async fn search_start_scan(
    request: SearchScanRequest,
    state: State<'_, KuraRuntime>,
) -> ApiResult<SearchStatus> {
    state.search.start_scan(request).await
}

#[tauri::command]
pub async fn search_cancel_scan(state: State<'_, KuraRuntime>) -> ApiResult<SearchStatus> {
    state.search.cancel_scan().await
}

#[tauri::command]
pub async fn search_query(
    request: SearchQueryRequest,
    state: State<'_, KuraRuntime>,
) -> ApiResult<Vec<SearchResult>> {
    state.search.query(request).await
}

#[tauri::command]
pub async fn explorer_prepare_open_item(
    request: PrepareOpenItemRequest,
    state: State<'_, KuraRuntime>,
) -> ApiResult<PreparedOpenItem> {
    state.explorer.prepare_open_item(request).await
}

#[tauri::command]
pub async fn explorer_prepare_drag_items(
    request: PrepareDragItemsRequest,
    state: State<'_, KuraRuntime>,
) -> ApiResult<PreparedDragItemsResult> {
    state.explorer.prepare_drag_items(request).await
}

#[tauri::command]
pub async fn explorer_cancel_drag_preparation(
    session_id: String,
    state: State<'_, KuraRuntime>,
) -> ApiResult<()> {
    state.explorer.cancel_drag_preparation(&session_id).await;
    Ok(())
}

#[tauri::command]
pub async fn explorer_preview_item(
    path: String,
    state: State<'_, KuraRuntime>,
) -> ApiResult<ExplorerPreviewPayload> {
    state.explorer.preview_item(&path).await
}

#[tauri::command]
pub async fn explorer_save_preview_item(
    request: SavePreviewRequest,
    state: State<'_, KuraRuntime>,
) -> ApiResult<ExplorerOperationResult> {
    state.explorer.save_preview_item(request).await
}

#[tauri::command]
pub async fn explorer_generate_image_thumbnail(
    path: String,
    max_dimension: u32,
    modified_ms: Option<u64>,
    remote_modified: Option<String>,
    size_bytes: Option<u64>,
    state: State<'_, KuraRuntime>,
) -> ApiResult<GeneratedImageThumbnail> {
    state
        .explorer
        .generate_image_thumbnail(
            &path,
            max_dimension,
            modified_ms,
            remote_modified.as_deref(),
            size_bytes,
        )
        .await
}

#[tauri::command]
pub async fn explorer_path_is_directory(
    path: String,
    state: State<'_, KuraRuntime>,
) -> ApiResult<bool> {
    Ok(state
        .explorer
        .item_is_directory(&path)
        .await?
        .unwrap_or(false))
}

#[tauri::command]
pub async fn explorer_path_exists(path: String, state: State<'_, KuraRuntime>) -> ApiResult<bool> {
    Ok(state.explorer.item_is_directory(&path).await?.is_some())
}

#[tauri::command]
pub async fn explorer_library_snapshot(
    state: State<'_, KuraRuntime>,
) -> ApiResult<ExplorerLibrarySnapshot> {
    state.explorer_library.snapshot().await
}

#[tauri::command]
pub async fn explorer_library_record_recent(
    request: RecordRecentRequest,
    state: State<'_, KuraRuntime>,
) -> ApiResult<ExplorerLibrarySnapshot> {
    state.explorer_library.record_recent(request).await
}

#[tauri::command]
pub async fn explorer_library_record_last_opened(
    request: RecordLastOpenedRequest,
    state: State<'_, KuraRuntime>,
) -> ApiResult<ExplorerLibrarySnapshot> {
    state.explorer_library.record_last_opened(request).await
}

#[tauri::command]
pub async fn explorer_open_with(application_path: String, file_path: String) -> ApiResult<()> {
    tokio::task::spawn_blocking(move || open_with_application(&application_path, &file_path))
        .await
        .map_err(|err| ApiError::Message(format!("Open With worker failed: {err}")))?
}

#[tauri::command]
pub async fn explorer_open_path(file_path: String) -> ApiResult<()> {
    tokio::task::spawn_blocking(move || open_path_default(&file_path))
        .await
        .map_err(|err| ApiError::Message(format!("Open file worker failed: {err}")))?
}

#[tauri::command]
pub async fn open_terminal_at_path(path: String, state: State<'_, KuraRuntime>) -> ApiResult<()> {
    let settings = state.settings.snapshot().await?;
    let preferred = settings
        .document
        .get("general")
        .and_then(|section| section.get("preferred_terminal_app"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("System Default")
        .to_owned();
    tokio::task::spawn_blocking(move || open_terminal_default(&path, &preferred))
        .await
        .map_err(|err| ApiError::Message(format!("Open terminal worker failed: {err}")))?
}

#[tauri::command]
pub async fn explorer_open_association(
    file_path: String,
    state: State<'_, KuraRuntime>,
) -> ApiResult<Option<String>> {
    state
        .settings
        .open_with_association_for_path(file_path)
        .await
}

#[tauri::command]
pub async fn settings_open_with_associations(
    state: State<'_, KuraRuntime>,
) -> ApiResult<Vec<OpenWithAssociation>> {
    state.settings.open_with_associations().await
}

#[tauri::command]
pub async fn explorer_queue_paste_items(
    request: PasteItemsRequest,
    state: State<'_, KuraRuntime>,
) -> ApiResult<OperationQueueSnapshot> {
    // Connected-device sources and destinations are resolved per item when the
    // queue runs, so remote moves and uploads keep their progress and conflicts.
    state.operation_queue.enqueue_paste_items(request).await
}

#[tauri::command]
pub async fn explorer_queue_paste_text(
    request: PasteTextRequest,
    state: State<'_, KuraRuntime>,
) -> ApiResult<OperationQueueSnapshot> {
    let paste_request = state.explorer.stage_clipboard_text_paste(request).await?;
    state
        .operation_queue
        .enqueue_paste_items(paste_request)
        .await
}

#[tauri::command]
pub async fn explorer_queue_paste_blob(
    request: PasteBlobRequest,
    state: State<'_, KuraRuntime>,
) -> ApiResult<OperationQueueSnapshot> {
    let paste_request = state.explorer.stage_clipboard_blob_paste(request).await?;
    state
        .operation_queue
        .enqueue_paste_items(paste_request)
        .await
}

#[tauri::command]
pub async fn explorer_queue_create_item(
    request: CreateItemRequest,
    state: State<'_, KuraRuntime>,
) -> ApiResult<OperationQueueSnapshot> {
    state.operation_queue.enqueue_create_item(request).await
}

#[tauri::command]
pub async fn explorer_queue_rename_item(
    request: RenameItemRequest,
    state: State<'_, KuraRuntime>,
) -> ApiResult<OperationQueueSnapshot> {
    state.operation_queue.enqueue_rename_item(request).await
}

#[tauri::command]
pub async fn explorer_queue_rename_items(
    request: RenameItemsRequest,
    state: State<'_, KuraRuntime>,
) -> ApiResult<OperationQueueSnapshot> {
    state.operation_queue.enqueue_rename_items(request).await
}

#[tauri::command]
pub async fn explorer_queue_delete_items(
    request: DeleteItemsRequest,
    state: State<'_, KuraRuntime>,
) -> ApiResult<OperationQueueSnapshot> {
    state.operation_queue.enqueue_delete_items(request).await
}

#[tauri::command]
pub async fn explorer_share(app: AppHandle, paths: Vec<String>, service: String) -> ApiResult<()> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let _ = sender.send(crate::infra::share::share_on_main_thread(&paths, &service));
    })
    .map_err(|err| ApiError::Message(format!("Could not share: {err}")))?;
    receiver
        .await
        .map_err(|err| ApiError::Message(format!("Could not share: {err}")))?
}

/// Finder tags for each of `paths`, in order. Read on demand for the rows on
/// screen: one getxattr costs about ten times an lstat, too much for every
/// entry of a large listing.
#[tauri::command]
pub async fn explorer_read_tags(
    paths: Vec<String>,
) -> ApiResult<Vec<Vec<crate::domain::finder_tags::FinderTag>>> {
    tokio::task::spawn_blocking(move || {
        paths
            .iter()
            .map(|path| crate::domain::finder_tags::read_tags(std::path::Path::new(path)))
            .collect()
    })
    .await
    .map_err(|err| ApiError::Message(format!("Tag worker failed: {err}")))
}

#[tauri::command]
pub async fn explorer_set_tag(paths: Vec<String>, name: String, enabled: bool) -> ApiResult<()> {
    tokio::task::spawn_blocking(move || crate::domain::finder_tags::set_tag(&paths, &name, enabled))
        .await
        .map_err(|err| ApiError::Message(format!("Tag worker failed: {err}")))?
}

#[tauri::command]
pub async fn explorer_volume_space(
    path: String,
) -> ApiResult<crate::infra::volume_space::VolumeSpace> {
    tokio::task::spawn_blocking(move || crate::infra::volume_space::volume_space(&path))
        .await
        .map_err(|err| ApiError::Message(format!("Free space worker failed: {err}")))?
}

#[tauri::command]
pub async fn explorer_put_back_items(
    paths: Vec<String>,
    state: State<'_, KuraRuntime>,
) -> ApiResult<Vec<String>> {
    state.explorer.put_back_items(paths).await
}

#[tauri::command]
pub async fn settings_snapshot(state: State<'_, KuraRuntime>) -> ApiResult<SettingsSnapshot> {
    state.settings.snapshot().await
}

#[tauri::command]
pub async fn settings_save(
    request: SaveSettingsRequest,
    state: State<'_, KuraRuntime>,
) -> ApiResult<SettingsSnapshot> {
    state.settings.save(request).await
}

#[tauri::command]
pub async fn devices_snapshot(state: State<'_, KuraRuntime>) -> ApiResult<DeviceSnapshot> {
    let devices = state.devices.clone();
    tokio::task::spawn_blocking(move || devices.snapshot())
        .await
        .map_err(|err| ApiError::Message(format!("Device scan failed: {err}")))
}

#[tauri::command]
pub async fn devices_unmount(
    request: DeviceUnmountRequest,
    state: State<'_, KuraRuntime>,
) -> ApiResult<DeviceSnapshot> {
    let devices = state.devices.clone();
    tokio::task::spawn_blocking(move || devices.unmount(request))
        .await
        .map_err(|err| ApiError::Message(format!("Device unmount failed: {err}")))?
}

#[tauri::command]
pub async fn transfers_snapshot(
    filter: Option<TransferFilter>,
    state: State<'_, KuraRuntime>,
) -> ApiResult<TransferPage> {
    state.transfers.snapshot(filter.unwrap_or_default()).await
}

fn open_with_application(application_path: &str, file_path: &str) -> ApiResult<()> {
    if application_path.trim().is_empty() || file_path.trim().is_empty() {
        return Err(ApiError::Message(
            "Application path and file path are required.".to_owned(),
        ));
    }

    #[cfg(target_os = "macos")]
    let spawn_result = Command::new("open")
        .arg("-a")
        .arg(application_path)
        .arg(file_path)
        .spawn();

    #[cfg(not(target_os = "macos"))]
    let spawn_result = Command::new(application_path).arg(file_path).spawn();

    spawn_result
        .map(|_| ())
        .map_err(|err| ApiError::Message(format!("Failed to open file with application: {err}")))
}

fn open_path_default(file_path: &str) -> ApiResult<()> {
    if file_path.trim().is_empty() {
        return Err(ApiError::Message("File path is required.".to_owned()));
    }

    // ShellExecute opens the file itself. `cmd /C start` would parse the path
    // as a command line, and file names may contain & | ^ that cmd runs.
    #[cfg(target_os = "windows")]
    {
        tauri_plugin_opener::open_path(file_path, None::<&str>)
            .map_err(|err| ApiError::Message(format!("Failed to open file: {err}")))
    }

    #[cfg(not(target_os = "windows"))]
    {
        #[cfg(target_os = "macos")]
        let spawn_result = Command::new("open").arg(file_path).spawn();

        #[cfg(all(unix, not(target_os = "macos")))]
        let spawn_result = Command::new("xdg-open").arg(file_path).spawn();

        spawn_result
            .map(|_| ())
            .map_err(|err| ApiError::Message(format!("Failed to open file: {err}")))
    }
}

fn open_terminal_default(path: &str, preferred: &str) -> ApiResult<()> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(ApiError::Message("Folder path is required.".to_owned()));
    }
    let folder = Path::new(trimmed);
    if !folder.is_dir() {
        return Err(ApiError::Message(format!("{trimmed} is not a folder.")));
    }

    #[cfg(target_os = "macos")]
    {
        let application = match preferred {
            "iTerm" => "iTerm",
            "Warp" => "Warp",
            "Ghostty" => "Ghostty",
            "Alacritty" => "Alacritty",
            _ => "Terminal",
        };
        Command::new("open")
            .arg("-a")
            .arg(application)
            .arg(trimmed)
            .spawn()
            .map(|_| ())
            .map_err(|err| ApiError::Message(format!("Failed to open {application}: {err}")))
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
        let executable = match preferred {
            "Warp" => "warp",
            "Ghostty" => "ghostty",
            "Alacritty" => "alacritty",
            _ => "wt",
        };
        // The folder is the working directory, never an argument: Windows
        // Terminal splits its arguments at `;`, and cmd runs & | ^ as commands.
        match Command::new(executable)
            .args(["-d", "."])
            .current_dir(folder)
            .spawn()
        {
            Ok(_) => Ok(()),
            Err(wt_err) => Command::new("cmd")
                .arg("/K")
                .current_dir(folder)
                .creation_flags(CREATE_NEW_CONSOLE)
                .spawn()
                .map(|_| ())
                .map_err(|cmd_err| {
                    ApiError::Message(format!(
                        "Failed to open Windows Terminal ({wt_err}) or Command Prompt ({cmd_err})."
                    ))
                }),
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let mut candidates = Vec::<String>::new();
        match preferred {
            "Warp" => candidates.push("warp-terminal".to_owned()),
            "Ghostty" => candidates.push("ghostty".to_owned()),
            "Alacritty" => candidates.push("alacritty".to_owned()),
            _ => {}
        }
        if let Ok(terminal) = env::var("TERMINAL") {
            let terminal = terminal.trim();
            if !terminal.is_empty() {
                candidates.push(terminal.to_owned());
            }
        }
        candidates.extend([
            "x-terminal-emulator".to_owned(),
            "gnome-terminal".to_owned(),
            "konsole".to_owned(),
            "xfce4-terminal".to_owned(),
            "alacritty".to_owned(),
            "kitty".to_owned(),
            "xterm".to_owned(),
        ]);

        let mut errors = Vec::new();
        for candidate in candidates {
            match Command::new(&candidate).current_dir(folder).spawn() {
                Ok(_) => return Ok(()),
                Err(err) => errors.push(format!("{candidate}: {err}")),
            }
        }
        Err(ApiError::Message(format!(
            "Failed to open a terminal for {trimmed}. Tried: {}",
            errors.join("; ")
        )))
    }
}

#[tauri::command]
pub async fn operation_queue_snapshot(
    state: State<'_, KuraRuntime>,
) -> ApiResult<OperationQueueSnapshot> {
    Ok(state.operation_queue.snapshot().await)
}

#[tauri::command]
pub async fn operation_queue_cancel(
    operation_id: u64,
    state: State<'_, KuraRuntime>,
) -> ApiResult<OperationQueueSnapshot> {
    state.operation_queue.cancel(operation_id).await
}

#[tauri::command]
pub async fn operation_queue_retry_transfer(
    transfer_id: u64,
    state: State<'_, KuraRuntime>,
) -> ApiResult<OperationQueueSnapshot> {
    state.operation_queue.retry_transfer(transfer_id).await
}

#[tauri::command]
pub async fn operation_queue_pause(
    operation_id: u64,
    state: State<'_, KuraRuntime>,
) -> ApiResult<OperationQueueSnapshot> {
    state.operation_queue.pause(operation_id).await
}

#[tauri::command]
pub async fn operation_queue_resume(
    operation_id: u64,
    state: State<'_, KuraRuntime>,
) -> ApiResult<OperationQueueSnapshot> {
    state.operation_queue.resume(operation_id).await
}

#[tauri::command]
pub async fn operation_queue_pause_all(
    state: State<'_, KuraRuntime>,
) -> ApiResult<OperationQueueSnapshot> {
    Ok(state.operation_queue.pause_all().await)
}

#[tauri::command]
pub async fn operation_queue_resume_all(
    state: State<'_, KuraRuntime>,
) -> ApiResult<OperationQueueSnapshot> {
    Ok(state.operation_queue.resume_all().await)
}

#[tauri::command]
pub async fn operation_queue_undo(
    undo_token_id: u64,
    state: State<'_, KuraRuntime>,
) -> ApiResult<OperationQueueSnapshot> {
    state.operation_queue.undo(undo_token_id).await
}

#[tauri::command]
pub async fn operation_queue_redo(
    state: State<'_, KuraRuntime>,
) -> ApiResult<OperationQueueSnapshot> {
    state.operation_queue.redo().await
}

#[tauri::command]
pub async fn operation_queue_resolve_conflict(
    operation_id: u64,
    policy: ConflictPolicy,
    apply_to_batch: bool,
    state: State<'_, KuraRuntime>,
) -> ApiResult<OperationQueueSnapshot> {
    state
        .operation_queue
        .resolve_conflict(operation_id, policy, apply_to_batch)
        .await
}

#[tauri::command]
pub async fn file_sync_pairs_snapshot(
    state: State<'_, KuraRuntime>,
) -> ApiResult<Vec<FileSyncPair>> {
    state.file_sync.pairs_snapshot().await
}

#[tauri::command]
pub async fn file_sync_pair_save(
    pair: FileSyncPair,
    state: State<'_, KuraRuntime>,
) -> ApiResult<FileSyncPair> {
    state.file_sync.save_pair(pair).await
}

#[tauri::command]
pub async fn file_sync_pair_remove(pair_id: i64, state: State<'_, KuraRuntime>) -> ApiResult<()> {
    state.file_sync.remove_pair(pair_id).await
}

#[tauri::command]
pub async fn file_sync_compare(
    request: FileSyncCompareRequest,
    state: State<'_, KuraRuntime>,
) -> ApiResult<crate::domain::file_sync::FileSyncCompareResult> {
    Ok(state.file_sync.compare(request).await)
}

#[tauri::command]
pub async fn file_sync_apply(
    request: FileSyncApplyRequest,
    state: State<'_, KuraRuntime>,
) -> ApiResult<FileSyncApplyResult> {
    state.file_sync.apply(request).await
}

#[tauri::command]
pub async fn archive_list(
    request: ArchiveListRequest,
    state: State<'_, KuraRuntime>,
) -> ApiResult<ArchiveListResult> {
    state.power_pack.archive_list(request).await
}

#[tauri::command]
pub async fn archive_create(
    request: ArchiveCreateRequest,
    state: State<'_, KuraRuntime>,
) -> ApiResult<ArchiveActionResult> {
    state.power_pack.archive_create(request).await
}

#[tauri::command]
pub async fn archive_extract(
    request: ArchiveExtractRequest,
    state: State<'_, KuraRuntime>,
) -> ApiResult<ArchiveActionResult> {
    state.power_pack.archive_extract(request).await
}

#[tauri::command]
pub async fn duplicates_scan(
    request: DuplicateScanRequest,
    state: State<'_, KuraRuntime>,
) -> ApiResult<DuplicateScanResult> {
    state.power_pack.duplicates_scan(request).await
}

#[tauri::command]
pub async fn duplicates_cancel(scan_id: String, state: State<'_, KuraRuntime>) -> ApiResult<bool> {
    state.power_pack.duplicates_cancel(scan_id).await
}

#[tauri::command]
pub async fn saved_searches_snapshot(
    state: State<'_, KuraRuntime>,
) -> ApiResult<SavedSearchesSnapshot> {
    state.power_pack.saved_searches_snapshot().await
}

#[tauri::command]
pub async fn saved_searches_save(
    search: SavedSearch,
    state: State<'_, KuraRuntime>,
) -> ApiResult<SavedSearchesSnapshot> {
    state.power_pack.saved_searches_save(search).await
}

#[tauri::command]
pub async fn saved_searches_delete(
    id: String,
    state: State<'_, KuraRuntime>,
) -> ApiResult<SavedSearchesSnapshot> {
    state.power_pack.saved_searches_delete(id).await
}

#[tauri::command]
pub async fn compare_files(
    request: CompareFilesRequest,
    state: State<'_, KuraRuntime>,
) -> ApiResult<CompareFilesResult> {
    state.power_pack.compare_files(request).await
}

#[tauri::command]
pub async fn compare_folders(
    request: CompareFoldersRequest,
    state: State<'_, KuraRuntime>,
) -> ApiResult<CompareFoldersResult> {
    state.power_pack.compare_folders(request).await
}

#[tauri::command]
pub async fn compare_apply_text_merge(
    merged_text: String,
    target_path: String,
    state: State<'_, KuraRuntime>,
) -> ApiResult<FileToolsActionResult> {
    state
        .power_pack
        .compare_apply_text_merge(merged_text, target_path)
        .await
}

#[tauri::command]
pub async fn file_tools_checksum(
    request: FileToolsChecksumRequest,
    state: State<'_, KuraRuntime>,
) -> ApiResult<FileToolsChecksumResult> {
    state.power_pack.file_tools_checksum(request).await
}

#[tauri::command]
pub async fn file_tools_create_symlink(
    request: FileToolsSymlinkRequest,
    state: State<'_, KuraRuntime>,
) -> ApiResult<FileToolsActionResult> {
    state.power_pack.file_tools_create_symlink(request).await
}

#[tauri::command]
pub async fn file_tools_read_symlink(
    request: FileToolsSymlinkTargetRequest,
    state: State<'_, KuraRuntime>,
) -> ApiResult<FileToolsSymlinkTargetResult> {
    state.power_pack.file_tools_read_symlink(request).await
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct Placeholder {}

#[tauri::command]
pub async fn navigation_names_snapshot(
    webview: tauri::Webview,
    state: State<'_, KuraRuntime>,
    account: String,
) -> Result<crate::infra::navigation_names::NavigationSnapshot, String> {
    if webview.label() != "main" {
        return Err("Only the Kura window can read navigation names.".into());
    }
    Ok(state.navigation_names.snapshot(&account).await)
}
#[tauri::command]
pub async fn navigation_names_update(
    webview: tauri::Webview,
    state: State<'_, KuraRuntime>,
    account: String,
    key: String,
    name: Option<String>,
) -> Result<crate::infra::navigation_names::NavigationSnapshot, String> {
    if webview.label() != "main" {
        return Err("Only the Kura window can rename navigation.".into());
    }
    state.navigation_names.update(account, key, name).await
}
