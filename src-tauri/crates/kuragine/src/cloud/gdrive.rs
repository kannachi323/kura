//! Google Drive v3. Items are addressed by ID; one name can appear twice in a
//! folder, and Docs/Sheets/Slides have no bytes of their own, so Kura exports
//! them to Office formats.
use std::path::Path;

use reqwest::{header, StatusCode};
use serde_json::{json, Value};

use super::{
    ensure_running,
    http::{read_chunk, write_body, Api},
    parse_time_ms, Account, Downloaded, Quota, RemoteItem, TransferObserver, UploadTarget,
};
use crate::{Error, Result};

const API: &str = "https://www.googleapis.com/drive/v3";
const UPLOAD: &str = "https://www.googleapis.com/upload/drive/v3";
const FIELDS: &str = "id,name,mimeType,size,modifiedTime,md5Checksum,version,trashed,shortcutDetails(targetId,targetMimeType)";
const FOLDER_MIME: &str = "application/vnd.google-apps.folder";
const SHORTCUT_MIME: &str = "application/vnd.google-apps.shortcut";
/// Multiple of 256 KiB, as resumable uploads require.
const CHUNK: u64 = 32 * 256 * 1024;

pub(super) fn export_extension(export_mime: &str) -> Option<&'static str> {
    match export_mime {
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => Some("docx"),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => Some("xlsx"),
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => Some("pptx"),
        "image/png" => Some("png"),
        _ => None,
    }
}

/// The export format for a Google-native type, or `None` if Kura can't export it.
fn export_mime(google_mime: &str) -> Option<&'static str> {
    match google_mime {
        "application/vnd.google-apps.document" => {
            Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
        }
        "application/vnd.google-apps.spreadsheet" => {
            Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
        }
        "application/vnd.google-apps.presentation" => {
            Some("application/vnd.openxmlformats-officedocument.presentationml.presentation")
        }
        "application/vnd.google-apps.drawing" => Some("image/png"),
        _ => None,
    }
}

/// Converts a Drive file resource. Returns `None` for things Kura can't show
/// as files: Forms, Sites, Maps and other Google-native types without an export.
fn to_item(value: &Value) -> Option<RemoteItem> {
    let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
    let mime = text("mimeType").unwrap_or_default();
    let mut item = RemoteItem {
        id: text("id")?,
        name: text("name").unwrap_or_default(),
        size: text("size").and_then(|size| size.parse().ok()).unwrap_or(0),
        modified_ms: text("modifiedTime").as_deref().and_then(parse_time_ms),
        revision: text("version"),
        hash: text("md5Checksum"),
        ..RemoteItem::default()
    };
    let content_mime = if mime == SHORTCUT_MIME {
        let details = value.get("shortcutDetails")?;
        item.target_id = details
            .get("targetId")
            .and_then(Value::as_str)
            .map(str::to_owned);
        details
            .get("targetMimeType")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    } else {
        mime
    };
    if content_mime == FOLDER_MIME {
        item.is_dir = true;
    } else if content_mime.starts_with("application/vnd.google-apps.") {
        item.export_mime = Some(export_mime(&content_mime)?.to_owned());
        item.mime_type = item.export_mime.clone();
        item.size = 0;
        item.hash = None;
    } else {
        item.mime_type = Some(content_mime);
    }
    Some(item)
}

fn require_item(value: &Value) -> Result<RemoteItem> {
    to_item(value).ok_or_else(|| Error::Decode("Google Drive returned an unsupported item.".into()))
}

pub(super) async fn account(api: &Api) -> Result<Account> {
    let about: Value = api
        .json(|client| {
            client
                .get(format!("{API}/about"))
                .query(&[("fields", "user(displayName,emailAddress)")])
        })
        .await?;
    Ok(Account {
        email: about
            .pointer("/user/emailAddress")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        display_name: about
            .pointer("/user/displayName")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        drive_type: String::new(),
    })
}

pub(super) async fn quota(api: &Api) -> Result<Quota> {
    let about: Value = api
        .json(|client| {
            client
                .get(format!("{API}/about"))
                .query(&[("fields", "storageQuota(limit,usage)")])
        })
        .await?;
    let number = |pointer: &str| {
        about
            .pointer(pointer)
            .and_then(Value::as_str)
            .and_then(|value| value.parse().ok())
            .unwrap_or(0)
    };
    Ok(Quota {
        used_bytes: number("/storageQuota/usage"),
        total_bytes: number("/storageQuota/limit"),
    })
}

pub(super) async fn root(api: &Api) -> Result<RemoteItem> {
    let value: Value = api
        .json(|client| {
            client
                .get(format!("{API}/files/root"))
                .query(&[("fields", FIELDS)])
        })
        .await?;
    require_item(&value)
}

pub(super) async fn list(api: &Api, folder: &RemoteItem) -> Result<Vec<RemoteItem>> {
    let query = format!(
        "'{}' in parents and trashed = false",
        folder
            .content_id()
            .replace('\\', "\\\\")
            .replace('\'', "\\'")
    );
    let fields = format!("nextPageToken,files({FIELDS})");
    let mut items = Vec::new();
    let mut page_token = String::new();
    loop {
        let page: Value = api
            .json(|client| {
                let mut request = client.get(format!("{API}/files")).query(&[
                    ("q", query.as_str()),
                    ("fields", fields.as_str()),
                    ("pageSize", "1000"),
                    ("supportsAllDrives", "true"),
                    ("includeItemsFromAllDrives", "true"),
                ]);
                if !page_token.is_empty() {
                    request = request.query(&[("pageToken", page_token.as_str())]);
                }
                request
            })
            .await?;
        items.extend(
            page.get("files")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(to_item),
        );
        match page.get("nextPageToken").and_then(Value::as_str) {
            Some(token) if !token.is_empty() => page_token = token.to_owned(),
            _ => return Ok(items),
        }
    }
}

async fn get(api: &Api, id: &str) -> Result<Value> {
    let value: Value = api
        .json(|client| {
            client
                .get(format!("{API}/files/{id}"))
                .query(&[("fields", FIELDS), ("supportsAllDrives", "true")])
        })
        .await?;
    if value.get("trashed").and_then(Value::as_bool) == Some(true) {
        return Err(Error::NotFound(id.to_owned()));
    }
    Ok(value)
}

pub(super) async fn stat(api: &Api, item: &RemoteItem) -> Result<RemoteItem> {
    let mut current = require_item(&get(api, &item.id).await?)?;
    // A shortcut's own metadata says nothing about the file it points to.
    if let Some(target) = current.target_id.clone().filter(|_| !current.is_dir) {
        let target = require_item(&get(api, &target).await?)?;
        current.size = target.size;
        current.hash = target.hash;
        current.revision = target.revision;
        current.modified_ms = target.modified_ms;
        current.mime_type = target.mime_type;
        current.export_mime = target.export_mime;
    }
    Ok(current)
}

pub(super) async fn download(
    api: &Api,
    item: &RemoteItem,
    destination: &Path,
    observer: &dyn TransferObserver,
) -> Result<Downloaded> {
    let id = item.content_id();
    let response = match &item.export_mime {
        Some(export) => {
            api.send(|client| {
                client
                    .get(format!("{API}/files/{id}/export"))
                    .query(&[("mimeType", export.as_str())])
            })
            .await?
        }
        None => {
            api.send(|client| {
                client
                    .get(format!("{API}/files/{id}"))
                    .query(&[("alt", "media"), ("supportsAllDrives", "true")])
            })
            .await?
        }
    };
    write_body(api.kind, response, destination, observer).await
}

pub(super) async fn upload(
    api: &Api,
    target: UploadTarget<'_>,
    source: &Path,
    observer: &dyn TransferObserver,
) -> Result<RemoteItem> {
    let size = tokio::fs::metadata(source).await?.len();
    let session_url = {
        let response = match target.existing {
            Some((existing, _)) => {
                let id = existing.content_id().to_owned();
                api.send(|client| {
                    client
                        .patch(format!("{UPLOAD}/files/{id}"))
                        .query(&[
                            ("uploadType", "resumable"),
                            ("supportsAllDrives", "true"),
                            ("fields", FIELDS),
                        ])
                        .header("X-Upload-Content-Length", size)
                        .json(&json!({}))
                })
                .await?
            }
            None => {
                let metadata = json!({
                    "name": target.name,
                    "parents": [target.parent.content_id()],
                });
                api.send(|client| {
                    client
                        .post(format!("{UPLOAD}/files"))
                        .query(&[
                            ("uploadType", "resumable"),
                            ("supportsAllDrives", "true"),
                            ("fields", FIELDS),
                        ])
                        .header("X-Upload-Content-Length", size)
                        .json(&metadata)
                })
                .await?
            }
        };
        response
            .headers()
            .get(header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
            .ok_or_else(|| Error::Decode("Google Drive did not start an upload session.".into()))?
    };

    let mut file = tokio::fs::File::open(source).await?;
    let mut offset = 0u64;
    loop {
        ensure_running(observer)?;
        let length = CHUNK.min(size - offset);
        let chunk = read_chunk(&mut file, length as usize).await?;
        let range = if size == 0 {
            "bytes */0".to_owned()
        } else {
            format!("bytes {}-{}/{}", offset, offset + length - 1, size)
        };
        let response = api
            .send(|_| {
                api.no_redirect
                    .put(&session_url)
                    .header(header::CONTENT_RANGE, range.as_str())
                    .body(chunk.clone())
            })
            .await?;
        if response.status() == StatusCode::PERMANENT_REDIRECT {
            // 308: Drive has the bytes up to the end of its Range header.
            let sent = offset + length;
            offset = response
                .headers()
                .get(header::RANGE)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.rsplit('-').next())
                .and_then(|end| end.parse::<u64>().ok())
                .map(|end| end + 1)
                .unwrap_or(0);
            if offset != sent {
                // Drive kept less than was sent; resume from what it has.
                file = tokio::fs::File::open(source).await?;
                tokio::io::AsyncSeekExt::seek(&mut file, std::io::SeekFrom::Start(offset)).await?;
            }
            observer.progress(offset, size);
            continue;
        }
        observer.progress(size, size);
        let value: Value = response.json().await?;
        return require_item(&value);
    }
}

pub(super) async fn create_folder(
    api: &Api,
    parent: &RemoteItem,
    name: &str,
) -> Result<RemoteItem> {
    let body = json!({ "name": name, "mimeType": FOLDER_MIME, "parents": [parent.content_id()] });
    let value: Value = api
        .json(|client| {
            client
                .post(format!("{API}/files"))
                .query(&[("fields", FIELDS), ("supportsAllDrives", "true")])
                .json(&body)
        })
        .await?;
    require_item(&value)
}

pub(super) async fn rename_move(
    api: &Api,
    item: &RemoteItem,
    new_name: &str,
    new_parent: Option<(&RemoteItem, &RemoteItem)>,
) -> Result<RemoteItem> {
    let body = json!({ "name": new_name });
    let mut query = vec![
        ("fields", FIELDS.to_owned()),
        ("supportsAllDrives", "true".to_owned()),
    ];
    if let Some((to, from)) = new_parent {
        query.push(("addParents", to.content_id().to_owned()));
        query.push(("removeParents", from.content_id().to_owned()));
    }
    let value: Value = api
        .json(|client| {
            client
                .patch(format!("{API}/files/{}", item.id))
                .query(&query)
                .json(&body)
        })
        .await?;
    require_item(&value)
}

pub(super) async fn copy_file(
    api: &Api,
    item: &RemoteItem,
    parent: &RemoteItem,
    name: &str,
) -> Result<RemoteItem> {
    let body = json!({ "name": name, "parents": [parent.content_id()] });
    let value: Value = api
        .json(|client| {
            client
                .post(format!("{API}/files/{}/copy", item.content_id()))
                .query(&[("fields", FIELDS), ("supportsAllDrives", "true")])
                .json(&body)
        })
        .await?;
    require_item(&value)
}

pub(super) async fn trash(api: &Api, item: &RemoteItem) -> Result<()> {
    api.send(|client| {
        client
            .patch(format!("{API}/files/{}", item.id))
            .query(&[("supportsAllDrives", "true")])
            .json(&json!({ "trashed": true }))
    })
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_files_folders_and_documents() {
        let file = to_item(&json!({
            "id": "1", "name": "a.pdf", "mimeType": "application/pdf", "size": "12",
            "md5Checksum": "abc", "version": "7", "modifiedTime": "2026-01-02T03:04:05.000Z"
        }))
        .unwrap();
        assert!(!file.is_dir);
        assert_eq!(file.size, 12);
        assert_eq!(file.hash.as_deref(), Some("abc"));
        assert_eq!(file.revision.as_deref(), Some("7"));
        assert!(file.modified_ms.is_some());

        let folder = to_item(&json!({"id": "2", "name": "Docs", "mimeType": FOLDER_MIME})).unwrap();
        assert!(folder.is_dir);

        let doc = to_item(&json!({
            "id": "3", "name": "Plan", "mimeType": "application/vnd.google-apps.document"
        }))
        .unwrap();
        assert_eq!(doc.export_extension(), Some("docx"));
        assert!(doc.hash.is_none());

        assert!(to_item(&json!({
            "id": "4", "name": "Survey", "mimeType": "application/vnd.google-apps.form"
        }))
        .is_none());
    }

    #[test]
    fn shortcuts_point_at_their_target() {
        let shortcut = to_item(&json!({
            "id": "s", "name": "Shared", "mimeType": SHORTCUT_MIME,
            "shortcutDetails": {"targetId": "t", "targetMimeType": FOLDER_MIME}
        }))
        .unwrap();
        assert!(shortcut.is_dir);
        assert_eq!(shortcut.content_id(), "t");
        assert_eq!(shortcut.id, "s");
    }
}
