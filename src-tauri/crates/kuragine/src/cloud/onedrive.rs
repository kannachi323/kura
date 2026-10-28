//! OneDrive through Microsoft Graph.
use std::{path::Path, time::Duration};

use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use reqwest::{header, StatusCode};
use serde_json::{json, Value};

use super::{
    ensure_running,
    http::{read_chunk, write_body, Api},
    parse_time_ms, Account, Downloaded, Quota, RemoteItem, TransferObserver, UploadTarget,
};
use crate::{Error, Result};

const DRIVE: &str = "https://graph.microsoft.com/v1.0/me/drive";
const SELECT: &str = "id,name,size,lastModifiedDateTime,eTag,file,folder,package,parentReference";
/// Graph's simple upload accepts up to 4 MiB.
const SIMPLE_UPLOAD_LIMIT: u64 = 4 * 1024 * 1024;
/// Multiple of 320 KiB, as upload sessions require.
const CHUNK: u64 = 32 * 320 * 1024;
const COPY_TIMEOUT: Duration = Duration::from_secs(600);

/// Characters to escape inside a `:/name:` path segment.
const SEGMENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'/')
    .add(b':')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'\\')
    .add(b'`')
    .add(b'{')
    .add(b'}');

fn to_item(value: &Value) -> Option<RemoteItem> {
    // OneNote notebooks are packages: neither a folder nor a downloadable file.
    if value.get("package").is_some() {
        return None;
    }
    let text = |pointer: &str| {
        value
            .pointer(pointer)
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    let is_dir = value.get("folder").is_some();
    Some(RemoteItem {
        id: text("/id")?,
        name: text("/name").unwrap_or_default(),
        is_dir,
        size: if is_dir {
            0
        } else {
            value.get("size").and_then(Value::as_u64).unwrap_or(0)
        },
        modified_ms: text("/lastModifiedDateTime")
            .as_deref()
            .and_then(parse_time_ms),
        mime_type: text("/file/mimeType"),
        revision: text("/eTag"),
        hash: text("/file/hashes/quickXorHash"),
        ..RemoteItem::default()
    })
}

fn require_item(value: &Value) -> Result<RemoteItem> {
    to_item(value).ok_or_else(|| Error::Decode("OneDrive returned an unsupported item.".into()))
}

fn item_url(item: &RemoteItem) -> String {
    format!("{DRIVE}/items/{}", item.id)
}

fn child_url(parent: &RemoteItem, name: &str, action: &str) -> String {
    format!(
        "{DRIVE}/items/{}:/{}:/{action}",
        parent.id,
        utf8_percent_encode(name, SEGMENT)
    )
}

pub(super) async fn account(api: &Api) -> Result<Account> {
    let me: Value = api
        .json(|client| {
            client
                .get("https://graph.microsoft.com/v1.0/me")
                .query(&[("$select", "displayName,mail,userPrincipalName")])
        })
        .await?;
    let drive: Value = api
        .json(|client| client.get(DRIVE).query(&[("$select", "driveType")]))
        .await?;
    let text = |value: &Value, key: &str| {
        value
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let mail = text(&me, "mail");
    Ok(Account {
        email: if mail.is_empty() {
            text(&me, "userPrincipalName")
        } else {
            mail
        },
        display_name: text(&me, "displayName"),
        drive_type: text(&drive, "driveType"),
    })
}

pub(super) async fn quota(api: &Api) -> Result<Quota> {
    let drive: Value = api
        .json(|client| client.get(DRIVE).query(&[("$select", "quota")]))
        .await?;
    let number = |pointer: &str| drive.pointer(pointer).and_then(Value::as_u64).unwrap_or(0);
    Ok(Quota {
        used_bytes: number("/quota/used"),
        total_bytes: number("/quota/total"),
    })
}

pub(super) async fn root(api: &Api) -> Result<RemoteItem> {
    let value: Value = api
        .json(|client| {
            client
                .get(format!("{DRIVE}/root"))
                .query(&[("$select", SELECT)])
        })
        .await?;
    require_item(&value)
}

pub(super) async fn list(api: &Api, folder: &RemoteItem) -> Result<Vec<RemoteItem>> {
    let mut items = Vec::new();
    let mut next = Some(format!(
        "{}/children?$top=1000&$select={SELECT}",
        item_url(folder)
    ));
    while let Some(url) = next.take() {
        let page: Value = api.json(|client| client.get(&url)).await?;
        items.extend(
            page.get("value")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(to_item),
        );
        next = page
            .get("@odata.nextLink")
            .and_then(Value::as_str)
            .map(str::to_owned);
    }
    Ok(items)
}

pub(super) async fn stat(api: &Api, item: &RemoteItem) -> Result<RemoteItem> {
    let value: Value = api
        .json(|client| client.get(item_url(item)).query(&[("$select", SELECT)]))
        .await?;
    require_item(&value)
}

pub(super) async fn download(
    api: &Api,
    item: &RemoteItem,
    destination: &Path,
    observer: &dyn TransferObserver,
) -> Result<Downloaded> {
    // Graph answers with a redirect to a pre-signed URL; reqwest follows it and
    // drops the Authorization header on the way.
    let url = format!("{}/content", item_url(item));
    let response = api.send(|client| client.get(&url)).await?;
    write_body(api.kind, response, destination, observer).await
}

pub(super) async fn upload(
    api: &Api,
    target: UploadTarget<'_>,
    source: &Path,
    observer: &dyn TransferObserver,
) -> Result<RemoteItem> {
    let size = tokio::fs::metadata(source).await?.len();
    let if_match = target
        .existing
        .and_then(|(_, revision)| revision)
        .map(str::to_owned);
    if size < SIMPLE_UPLOAD_LIMIT {
        let body = tokio::fs::read(source).await?;
        let url = match target.existing {
            Some((existing, _)) => format!("{}/content", item_url(existing)),
            None => format!(
                "{}?@microsoft.graph.conflictBehavior=fail",
                child_url(target.parent, target.name, "content")
            ),
        };
        let value: Value = api
            .json(|client| {
                let mut request = client
                    .put(&url)
                    .header(header::CONTENT_TYPE, "application/octet-stream")
                    .body(body.clone());
                if let Some(etag) = &if_match {
                    request = request.header(header::IF_MATCH, etag);
                }
                request
            })
            .await
            .map_err(name_conflict)?;
        observer.progress(size, size);
        return require_item(&value);
    }

    let (url, behavior) = match target.existing {
        Some((existing, _)) => (
            format!("{}/createUploadSession", item_url(existing)),
            "replace",
        ),
        None => (
            child_url(target.parent, target.name, "createUploadSession"),
            "fail",
        ),
    };
    let session: Value = api
        .json(|client| {
            let mut request = client
                .post(&url)
                .json(&json!({ "item": { "@microsoft.graph.conflictBehavior": behavior } }));
            if let Some(etag) = &if_match {
                request = request.header(header::IF_MATCH, etag);
            }
            request
        })
        .await
        .map_err(name_conflict)?;
    let upload_url = session
        .get("uploadUrl")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::Decode("OneDrive did not start an upload session.".into()))?
        .to_owned();

    let mut file = tokio::fs::File::open(source).await?;
    let mut offset = 0u64;
    loop {
        ensure_running(observer)?;
        let length = CHUNK.min(size - offset);
        let chunk = read_chunk(&mut file, length as usize).await?;
        let range = format!("bytes {}-{}/{}", offset, offset + length - 1, size);
        let response = api
            .send_anonymous(|client| {
                client
                    .put(&upload_url)
                    .header(header::CONTENT_RANGE, range.as_str())
                    .body(chunk.clone())
            })
            .await
            .map_err(name_conflict)?;
        offset += length;
        observer.progress(offset, size);
        if response.status() == StatusCode::ACCEPTED {
            continue;
        }
        let value: Value = response.json().await?;
        return require_item(&value);
    }
}

/// Graph answers 409 when a new file's name is taken.
fn name_conflict(error: Error) -> Error {
    match error {
        Error::Http {
            status: 409,
            message,
            ..
        } => Error::Conflict(message),
        other => other,
    }
}

pub(super) async fn create_folder(
    api: &Api,
    parent: &RemoteItem,
    name: &str,
) -> Result<RemoteItem> {
    let body = json!({
        "name": name,
        "folder": {},
        "@microsoft.graph.conflictBehavior": "fail",
    });
    let value: Value = api
        .json(|client| {
            client
                .post(format!("{}/children", item_url(parent)))
                .json(&body)
        })
        .await
        .map_err(name_conflict)?;
    require_item(&value)
}

pub(super) async fn rename_move(
    api: &Api,
    item: &RemoteItem,
    new_name: &str,
    new_parent: Option<(&RemoteItem, &RemoteItem)>,
) -> Result<RemoteItem> {
    let mut body = json!({ "name": new_name });
    if let Some((to, _)) = new_parent {
        body["parentReference"] = json!({ "id": to.id });
    }
    let value: Value = api
        .json(|client| client.patch(item_url(item)).json(&body))
        .await
        .map_err(name_conflict)?;
    require_item(&value)
}

pub(super) async fn copy_file(
    api: &Api,
    item: &RemoteItem,
    parent: &RemoteItem,
    name: &str,
) -> Result<RemoteItem> {
    let body = json!({ "parentReference": { "id": parent.id }, "name": name });
    let response = api
        .send(|client| client.post(format!("{}/copy", item_url(item))).json(&body))
        .await
        .map_err(name_conflict)?;
    // Copies run in the background; Graph returns a monitor URL to poll.
    let monitor = response
        .headers()
        .get(header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
        .ok_or_else(|| Error::Decode("OneDrive did not return a copy monitor.".into()))?;
    let started = std::time::Instant::now();
    loop {
        tokio::time::sleep(Duration::from_millis(800)).await;
        let status: Value = api
            .send_anonymous(|client| client.get(&monitor))
            .await?
            .json()
            .await?;
        match status.get("status").and_then(Value::as_str) {
            Some("completed") => {
                let id = status
                    .get("resourceId")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        Error::Decode("OneDrive copy finished without an item.".into())
                    })?;
                let copied = RemoteItem {
                    id: id.to_owned(),
                    ..RemoteItem::default()
                };
                return stat(api, &copied).await;
            }
            Some("failed") => {
                return Err(Error::Http {
                    provider: "OneDrive",
                    status: 500,
                    message: status
                        .pointer("/error/message")
                        .and_then(Value::as_str)
                        .unwrap_or("The copy failed.")
                        .to_owned(),
                })
            }
            _ if started.elapsed() > COPY_TIMEOUT => {
                return Err(Error::Network(
                    "OneDrive is still copying; try again later.".into(),
                ))
            }
            _ => {}
        }
    }
}

pub(super) async fn trash(api: &Api, item: &RemoteItem) -> Result<()> {
    api.send(|client| client.delete(item_url(item))).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_items_and_skips_notebooks() {
        let file = to_item(&json!({
            "id": "A!1", "name": "a.txt", "size": 5, "eTag": "\"{X},2\"",
            "lastModifiedDateTime": "2026-01-02T03:04:05Z",
            "file": {"mimeType": "text/plain", "hashes": {"quickXorHash": "abc="}}
        }))
        .unwrap();
        assert_eq!(file.hash.as_deref(), Some("abc="));
        assert_eq!(file.revision.as_deref(), Some("\"{X},2\""));
        assert!(!file.is_dir);

        let folder =
            to_item(&json!({"id": "A!2", "name": "Docs", "folder": {"childCount": 3}, "size": 99}))
                .unwrap();
        assert!(folder.is_dir);
        assert_eq!(folder.size, 0);

        assert!(
            to_item(&json!({"id": "A!3", "name": "Notebook", "package": {"type": "oneNote"}}))
                .is_none()
        );
    }

    #[test]
    fn encodes_names_in_path_segments() {
        let parent = RemoteItem {
            id: "P".into(),
            ..RemoteItem::default()
        };
        assert_eq!(
            child_url(&parent, "a b:c?.txt", "content"),
            format!("{DRIVE}/items/P:/a%20b%3Ac%3F.txt:/content")
        );
    }
}
