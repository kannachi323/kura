//! Cloud accounts: adding them with the person's own OAuth app, signing in,
//! and settling upload conflicts. Browsing and transfers go through the
//! explorer and operation queue commands with ordinary paths.
use kuragine::{AppCredentials, ConflictChoice, ConflictInfo, ProviderKind, Quota, RemoteInfo};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::app::runtime::KuraRuntime;
use crate::domain::operation_queue::{ConflictPolicy, OperationQueueSnapshot};
use crate::error::{ApiError, ApiResult};

pub const CLOUD_EVENT: &str = "kura://cloud";

#[tauri::command]
pub fn cloud_remotes(state: State<'_, KuraRuntime>) -> ApiResult<Vec<RemoteInfo>> {
    Ok(state.explorer.cloud().remotes()?)
}

#[tauri::command]
pub async fn cloud_add_remote(
    state: State<'_, KuraRuntime>,
    name: String,
    kind: ProviderKind,
    credentials: AppCredentials,
) -> ApiResult<RemoteInfo> {
    Ok(state
        .explorer
        .cloud()
        .add_remote(&name, kind, credentials)
        .await?)
}

#[tauri::command]
pub async fn cloud_set_credentials(
    state: State<'_, KuraRuntime>,
    name: String,
    credentials: AppCredentials,
) -> ApiResult<()> {
    Ok(state
        .explorer
        .cloud()
        .set_credentials(&name, credentials)
        .await?)
}

/// Opens the provider's sign-in page in the browser and waits for it.
#[tauri::command]
pub async fn cloud_sign_in(
    app: AppHandle,
    state: State<'_, KuraRuntime>,
    name: String,
) -> ApiResult<RemoteInfo> {
    let opener = app.clone();
    Ok(state
        .explorer
        .cloud()
        .sign_in(&name, move |url| {
            opener
                .opener()
                .open_url(url, None::<&str>)
                .map_err(|error| {
                    kuragine::Error::invalid(format!("Couldn't open the browser: {error}"))
                })
        })
        .await?)
}

#[tauri::command]
pub fn cloud_cancel_sign_in(state: State<'_, KuraRuntime>, name: String) {
    state.explorer.cloud().cancel_sign_in(&name);
}

#[tauri::command]
pub async fn cloud_remove_remote(
    state: State<'_, KuraRuntime>,
    name: String,
    delete_files: bool,
) -> ApiResult<()> {
    Ok(state
        .explorer
        .cloud()
        .remove_remote(&name, delete_files)
        .await?)
}

#[tauri::command]
pub async fn cloud_quota(state: State<'_, KuraRuntime>, name: String) -> ApiResult<Quota> {
    Ok(state.explorer.cloud().quota(&name).await?)
}

#[tauri::command]
pub fn cloud_conflicts(state: State<'_, KuraRuntime>) -> ApiResult<Vec<ConflictInfo>> {
    Ok(state.explorer.cloud().conflicts()?)
}

/// Settles an upload conflict. With the upload's queue operation, keeping
/// mine or both reruns that upload so it shows in Transfers; keeping the cloud
/// version settles it here and skips the upload.
#[tauri::command]
pub async fn cloud_resolve_conflict(
    state: State<'_, KuraRuntime>,
    path: String,
    choice: ConflictChoice,
    operation_id: Option<u64>,
) -> ApiResult<OperationQueueSnapshot> {
    let queue = &state.operation_queue;
    match (choice, operation_id.filter(|id| *id > 0)) {
        (ConflictChoice::KeepMine, Some(id)) => {
            queue
                .resolve_conflict(id, ConflictPolicy::Replace, false)
                .await
        }
        (ConflictChoice::KeepBoth, Some(id)) => {
            queue
                .resolve_conflict(id, ConflictPolicy::KeepBoth, false)
                .await
        }
        (choice, operation) => {
            state
                .explorer
                .cloud_resolve_conflict(&path, choice, None, 0)
                .await?;
            match operation {
                Some(id) => {
                    queue
                        .resolve_conflict(id, ConflictPolicy::Skip, false)
                        .await
                }
                None => Ok(queue.snapshot().await),
            }
        }
    }
}

/// Starts the cloud watcher and forwards Kuragine's events: to the window, and
/// changed files to the operation queue as uploads.
pub fn start(app: &AppHandle) {
    let Some(runtime) = app.try_state::<KuraRuntime>() else {
        return;
    };
    let engine = runtime.explorer.cloud().clone();
    let queue = runtime.operation_queue.clone();
    let mut events = engine.subscribe();
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            let event = match events.recv().await {
                Ok(event) => event,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
            };
            if let kuragine::EngineEvent::UploadNeeded { path, .. } = &event {
                if let Err(error) = queue.enqueue_cloud_upload(path.clone()).await {
                    eprintln!("kura: could not queue an upload of {path}: {error}");
                }
            }
            let _ = handle.emit(CLOUD_EVENT, &event);
        }
    });
    tauri::async_runtime::spawn(async move {
        if let Err(error) = engine.watch().await {
            eprintln!(
                "kura: cloud folders aren't being watched: {}",
                ApiError::from(error)
            );
        }
    });
}
