//! Kura's optional link to the Misty app on the same computer. When Misty is
//! running and the person has allowed it, Kura can show the clipboard they
//! share across their Misty devices and paste clips into folders. Kura never
//! signs in or talks to a network for this; it only uses Misty's local socket.
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MistyClipboard {
    /// "unavailable": Misty isn't running here. "denied": the person said no
    /// in Misty. "ready": `view` holds Misty's clipboard view.
    pub state: &'static str,
    pub view: Option<Value>,
}

#[cfg(unix)]
mod socket {
    use std::{os::unix::fs::PermissionsExt, path::PathBuf, time::Duration};

    use serde_json::{json, Value};
    use tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
        net::UnixStream,
        time::timeout,
    };

    /// Misty listens here (see its `clipboard_bridge.rs`).
    fn socket_path() -> Option<PathBuf> {
        dirs::data_dir().map(|dir| {
            dir.join("com.misty.desktop")
                .join("bridge")
                .join("clipboard.sock")
        })
    }

    fn token_path() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join("com.misty.kura").join("misty-bridge-token"))
    }

    fn saved_token() -> String {
        token_path()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .map(|token| token.trim().to_owned())
            .unwrap_or_default()
    }

    fn save_token(token: &str) {
        let Some(path) = token_path() else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if std::fs::write(&path, token).is_ok() {
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
    }

    pub enum Outcome {
        Unavailable,
        Denied,
        Ready(Value),
        Failed(String),
    }

    /// One request: say hello (Misty may ask the person once), then call.
    pub async fn request(method: &str, params: Value) -> Outcome {
        let Some(path) = socket_path() else {
            return Outcome::Unavailable;
        };
        let Ok(stream) = UnixStream::connect(&path).await else {
            return Outcome::Unavailable;
        };
        let (reader, mut writer) = stream.into_split();
        let mut lines = BufReader::new(reader).lines();
        let send = |id: u64, method: &str, params: Value| {
            let mut line =
                serde_json::to_vec(&json!({ "id": id, "method": method, "params": params }))
                    .unwrap_or_default();
            line.push(b'\n');
            line
        };
        // The first hello can wait for the person to answer Misty's prompt.
        let hello = send(1, "hello", json!({ "token": saved_token(), "app": "Kura" }));
        if writer.write_all(&hello).await.is_err() {
            return Outcome::Unavailable;
        }
        let Ok(Ok(Some(line))) = timeout(Duration::from_secs(180), lines.next_line()).await else {
            return Outcome::Unavailable;
        };
        let Ok(reply) = serde_json::from_str::<Value>(&line) else {
            return Outcome::Unavailable;
        };
        match reply["result"]["status"].as_str() {
            Some("approved") => {
                if let Some(token) = reply["result"]["token"].as_str() {
                    save_token(token);
                }
            }
            Some("denied") => return Outcome::Denied,
            _ => return Outcome::Unavailable,
        }
        if writer.write_all(&send(2, method, params)).await.is_err() {
            return Outcome::Unavailable;
        }
        let Ok(Ok(Some(line))) = timeout(Duration::from_secs(120), lines.next_line()).await else {
            return Outcome::Unavailable;
        };
        let Ok(reply) = serde_json::from_str::<Value>(&line) else {
            return Outcome::Unavailable;
        };
        if reply["ok"].as_bool() == Some(true) {
            Outcome::Ready(reply["result"].clone())
        } else {
            Outcome::Failed(
                reply["error"]
                    .as_str()
                    .unwrap_or("Misty couldn't do that.")
                    .to_owned(),
            )
        }
    }
}

#[cfg(not(unix))]
mod socket {
    use serde_json::Value;

    pub enum Outcome {
        Unavailable,
        Denied,
        Ready(Value),
        Failed(String),
    }

    /// The Windows bridge (a named pipe) isn't built yet.
    pub async fn request(_method: &str, _params: Value) -> Outcome {
        Outcome::Unavailable
    }
}

use socket::{request, Outcome};

#[tauri::command]
pub async fn misty_clipboard_view() -> MistyClipboard {
    match request("list", json!({})).await {
        Outcome::Ready(view) => MistyClipboard {
            state: "ready",
            view: Some(view),
        },
        Outcome::Denied => MistyClipboard {
            state: "denied",
            view: None,
        },
        Outcome::Unavailable | Outcome::Failed(_) => MistyClipboard {
            state: "unavailable",
            view: None,
        },
    }
}

/// Puts a clip on this computer's clipboard, through Misty.
#[tauri::command]
pub async fn misty_clipboard_copy(clip_id: String) -> Result<(), String> {
    match request("copy", json!({ "clipId": clip_id })).await {
        Outcome::Ready(_) => Ok(()),
        Outcome::Failed(error) => Err(error),
        Outcome::Denied => Err("Allow Kura in Misty to use your shared clipboard.".into()),
        Outcome::Unavailable => Err("Open Misty to use your shared clipboard.".into()),
    }
}

/// Writes a clip into `directory`: its files, or its text as a text file.
#[tauri::command]
pub async fn misty_clipboard_paste(
    clip_id: String,
    directory: String,
) -> Result<Vec<String>, String> {
    match request(
        "materialize",
        json!({ "clipId": clip_id, "directory": directory }),
    )
    .await
    {
        Outcome::Ready(result) => Ok(result["paths"]
            .as_array()
            .map(|paths| {
                paths
                    .iter()
                    .filter_map(|path| path.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()),
        Outcome::Failed(error) => Err(error),
        Outcome::Denied => Err("Allow Kura in Misty to use your shared clipboard.".into()),
        Outcome::Unavailable => Err("Open Misty to use your shared clipboard.".into()),
    }
}
