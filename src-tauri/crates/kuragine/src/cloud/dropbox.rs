//! Dropbox API v2. Items have stable `id:` IDs, but uploads and new folders are
//! addressed by path, so Kura keeps each item's `path_display` too.
use std::path::Path;

use reqwest::{header, Response};
use serde_json::{json, Value};

use super::{
    ensure_running,
    http::{read_chunk, write_body, Api},
    parse_time_ms, Account, Downloaded, Quota, RemoteItem, TransferObserver, UploadTarget,
};
use crate::{Error, Result};

const API: &str = "https://api.dropboxapi.com/2";
const CONTENT: &str = "https://content.dropboxapi.com/2";
const SIMPLE_UPLOAD_LIMIT: u64 = 8 * 1024 * 1024;
const CHUNK: u64 = 8 * 1024 * 1024;
const API_ARG: &str = "Dropbox-API-Arg";

fn to_item(value: &Value) -> Option<RemoteItem> {
    let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
    let is_dir = match text(".tag")?.as_str() {
        "file" => false,
        "folder" => true,
        _ => return None,
    };
    Some(RemoteItem {
        id: text("id")?,
        name: text("name").unwrap_or_default(),
        is_dir,
        size: value.get("size").and_then(Value::as_u64).unwrap_or(0),
        modified_ms: text("server_modified").as_deref().and_then(parse_time_ms),
        revision: text("rev"),
        hash: text("content_hash"),
        provider_path: text("path_display"),
        ..RemoteItem::default()
    })
}

fn require_item(value: &Value) -> Result<RemoteItem> {
    to_item(value).ok_or_else(|| Error::Decode("Dropbox returned an unsupported item.".into()))
}

/// The argument naming an item: its ID, or "" for the root.
fn path_arg(item: &RemoteItem) -> &str {
    if item.id.is_empty() {
        item.provider_path.as_deref().unwrap_or("")
    } else {
        &item.id
    }
}

fn child_path(parent: &RemoteItem, name: &str) -> String {
    format!("{}/{name}", parent.provider_path.as_deref().unwrap_or(""))
}

fn parent_path(item: &RemoteItem) -> String {
    let path = item.provider_path.as_deref().unwrap_or("");
    match path.rfind('/') {
        Some(index) => path[..index].to_owned(),
        None => String::new(),
    }
}

/// Dropbox reports endpoint errors as 409 with a summary like
/// `path/not_found/..` or `path/conflict/file/..`.
fn map_error(error: Error) -> Error {
    match error {
        Error::Http {
            status: 409,
            message,
            ..
        } if message.contains("not_found") => Error::NotFound(message),
        Error::Http {
            status: 409,
            message,
            ..
        } if message.contains("conflict") => Error::Conflict(message),
        other => other,
    }
}

/// `Dropbox-API-Arg` must be ASCII, so everything else is `\u` escaped.
fn header_json(value: &Value) -> String {
    let mut out = String::new();
    for unit in value.to_string().encode_utf16() {
        if unit < 0x80 {
            out.push(unit as u8 as char);
        } else {
            out.push_str(&format!("\\u{unit:04x}"));
        }
    }
    out
}

async fn rpc(api: &Api, endpoint: &str, body: Value) -> Result<Value> {
    api.json(|client| client.post(format!("{API}{endpoint}")).json(&body))
        .await
        .map_err(map_error)
}

async fn content(api: &Api, endpoint: &str, arg: Value, body: bytes::Bytes) -> Result<Response> {
    let arg = header_json(&arg);
    api.send(|client| {
        client
            .post(format!("{CONTENT}{endpoint}"))
            .header(API_ARG, arg.as_str())
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .body(body.clone())
    })
    .await
    .map_err(map_error)
}

pub(super) async fn account(api: &Api) -> Result<Account> {
    let value = rpc(api, "/users/get_current_account", Value::Null).await?;
    Ok(Account {
        email: value
            .get("email")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        display_name: value
            .pointer("/name/display_name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        drive_type: String::new(),
    })
}

pub(super) async fn quota(api: &Api) -> Result<Quota> {
    let value = rpc(api, "/users/get_space_usage", Value::Null).await?;
    Ok(Quota {
        used_bytes: value.get("used").and_then(Value::as_u64).unwrap_or(0),
        total_bytes: value
            .pointer("/allocation/allocated")
            .and_then(Value::as_u64)
            .unwrap_or(0),
    })
}

pub(super) async fn root(_api: &Api) -> Result<RemoteItem> {
    Ok(RemoteItem {
        id: String::new(),
        is_dir: true,
        provider_path: Some(String::new()),
        ..RemoteItem::default()
    })
}

pub(super) async fn list(api: &Api, folder: &RemoteItem) -> Result<Vec<RemoteItem>> {
    let mut page = rpc(
        api,
        "/files/list_folder",
        json!({ "path": path_arg(folder), "limit": 2000, "include_non_downloadable_files": false }),
    )
    .await?;
    let mut items = Vec::new();
    loop {
        items.extend(
            page.get("entries")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(to_item),
        );
        if page.get("has_more").and_then(Value::as_bool) != Some(true) {
            return Ok(items);
        }
        let cursor = page
            .get("cursor")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        page = rpc(
            api,
            "/files/list_folder/continue",
            json!({ "cursor": cursor }),
        )
        .await?;
    }
}

pub(super) async fn stat(api: &Api, item: &RemoteItem) -> Result<RemoteItem> {
    if item.id.is_empty() {
        return root(api).await;
    }
    let value = rpc(api, "/files/get_metadata", json!({ "path": item.id })).await?;
    require_item(&value)
}

pub(super) async fn download(
    api: &Api,
    item: &RemoteItem,
    destination: &Path,
    observer: &dyn TransferObserver,
) -> Result<Downloaded> {
    let arg = header_json(&json!({ "path": path_arg(item) }));
    let response = api
        .send(|client| {
            client
                .post(format!("{CONTENT}/files/download"))
                .header(API_ARG, arg.as_str())
        })
        .await
        .map_err(map_error)?;
    write_body(api.kind, response, destination, observer).await
}

pub(super) async fn upload(
    api: &Api,
    target: UploadTarget<'_>,
    source: &Path,
    observer: &dyn TransferObserver,
) -> Result<RemoteItem> {
    let size = tokio::fs::metadata(source).await?.len();
    let (path, mode) = match target.existing {
        Some((existing, revision)) => (
            existing
                .provider_path
                .clone()
                .unwrap_or_else(|| child_path(target.parent, target.name)),
            match revision {
                Some(rev) => json!({ ".tag": "update", "update": rev }),
                None => json!({ ".tag": "overwrite" }),
            },
        ),
        None => (
            child_path(target.parent, target.name),
            json!({ ".tag": "add" }),
        ),
    };
    let commit = json!({
        "path": path,
        "mode": mode,
        "autorename": false,
        "mute": true,
        "strict_conflict": true,
    });
    let mut file = tokio::fs::File::open(source).await?;
    if size <= SIMPLE_UPLOAD_LIMIT {
        let body = read_chunk(&mut file, size as usize).await?;
        let value: Value = content(api, "/files/upload", commit, body)
            .await?
            .json()
            .await?;
        observer.progress(size, size);
        return require_item(&value);
    }

    let first = read_chunk(&mut file, CHUNK as usize).await?;
    let started: Value = content(
        api,
        "/files/upload_session/start",
        json!({ "close": false }),
        first,
    )
    .await?
    .json()
    .await?;
    let session = started
        .get("session_id")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::Decode("Dropbox did not start an upload session.".into()))?
        .to_owned();
    let mut offset = CHUNK;
    observer.progress(offset, size);
    while size - offset > CHUNK {
        ensure_running(observer)?;
        let chunk = read_chunk(&mut file, CHUNK as usize).await?;
        content(
            api,
            "/files/upload_session/append_v2",
            json!({ "cursor": { "session_id": session, "offset": offset }, "close": false }),
            chunk,
        )
        .await?;
        offset += CHUNK;
        observer.progress(offset, size);
    }
    ensure_running(observer)?;
    let last = read_chunk(&mut file, (size - offset) as usize).await?;
    let value: Value = content(
        api,
        "/files/upload_session/finish",
        json!({ "cursor": { "session_id": session, "offset": offset }, "commit": commit }),
        last,
    )
    .await?
    .json()
    .await?;
    observer.progress(size, size);
    require_item(&value)
}

pub(super) async fn create_folder(
    api: &Api,
    parent: &RemoteItem,
    name: &str,
) -> Result<RemoteItem> {
    let value = rpc(
        api,
        "/files/create_folder_v2",
        json!({ "path": child_path(parent, name), "autorename": false }),
    )
    .await?;
    require_item(value.get("metadata").unwrap_or(&Value::Null))
}

pub(super) async fn rename_move(
    api: &Api,
    item: &RemoteItem,
    new_name: &str,
    new_parent: Option<(&RemoteItem, &RemoteItem)>,
) -> Result<RemoteItem> {
    let to_path = match new_parent {
        Some((to, _)) => child_path(to, new_name),
        None => format!("{}/{new_name}", parent_path(item)),
    };
    let value = rpc(
        api,
        "/files/move_v2",
        json!({ "from_path": path_arg(item), "to_path": to_path, "autorename": false }),
    )
    .await?;
    require_item(value.get("metadata").unwrap_or(&Value::Null))
}

pub(super) async fn copy_file(
    api: &Api,
    item: &RemoteItem,
    parent: &RemoteItem,
    name: &str,
) -> Result<RemoteItem> {
    let value = rpc(
        api,
        "/files/copy_v2",
        json!({ "from_path": path_arg(item), "to_path": child_path(parent, name), "autorename": false }),
    )
    .await?;
    require_item(value.get("metadata").unwrap_or(&Value::Null))
}

pub(super) async fn trash(api: &Api, item: &RemoteItem) -> Result<()> {
    rpc(api, "/files/delete_v2", json!({ "path": path_arg(item) })).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_entries() {
        let file = to_item(&json!({
            ".tag": "file", "id": "id:1", "name": "a.txt", "path_display": "/Docs/a.txt",
            "size": 3, "rev": "015", "content_hash": "ff", "server_modified": "2026-01-02T03:04:05Z"
        }))
        .unwrap();
        assert_eq!(file.revision.as_deref(), Some("015"));
        assert_eq!(parent_path(&file), "/Docs");
        assert!(to_item(&json!({".tag": "deleted", "name": "x"})).is_none());
    }

    #[test]
    fn escapes_non_ascii_in_api_arg_header() {
        assert_eq!(
            header_json(&json!({"path": "/\u{e9}"})),
            "{\"path\":\"/\\u00e9\"}"
        );
        assert_eq!(
            header_json(&json!({"path": "/\u{1f600}"})),
            "{\"path\":\"/\\ud83d\\ude00\"}"
        );
    }

    #[test]
    fn maps_endpoint_errors() {
        let error = map_error(Error::Http {
            provider: "Dropbox",
            status: 409,
            message: "path/conflict/file/..".into(),
        });
        assert!(matches!(error, Error::Conflict(_)));
        let error = map_error(Error::Http {
            provider: "Dropbox",
            status: 409,
            message: "path/not_found/..".into(),
        });
        assert!(error.is_not_found());
    }

    #[test]
    fn root_children_are_addressed_from_the_empty_path() {
        let root = RemoteItem {
            provider_path: Some(String::new()),
            is_dir: true,
            ..RemoteItem::default()
        };
        assert_eq!(child_path(&root, "a.txt"), "/a.txt");
        assert_eq!(path_arg(&root), "");
    }
}
