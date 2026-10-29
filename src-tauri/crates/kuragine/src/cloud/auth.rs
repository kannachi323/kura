//! OAuth with the person's own app registration: authorization code + PKCE,
//! redirected to a one-shot listener on the loopback interface.
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Mutex,
};
use url::Url;

use super::{
    secrets::{RemoteSecret, SecretStore},
    ProviderKind,
};
use crate::{Error, Result};

/// Dropbox matches redirect URIs exactly, so its listener uses a fixed port.
pub const DROPBOX_REDIRECT_PORT: u16 = 53682;
const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(300);
const REFRESH_MARGIN: Duration = Duration::from_secs(90);

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppCredentials {
    pub client_id: String,
    /// Google desktop clients have one; OneDrive and Dropbox with PKCE don't.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
    /// Microsoft tenant: "common" (default), "consumers", "organizations" or a
    /// tenant ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant: Option<String>,
}

impl AppCredentials {
    pub fn validate(&self, kind: ProviderKind) -> Result<()> {
        if self.client_id.trim().is_empty() {
            return Err(Error::invalid("Enter the app's client ID."));
        }
        if kind == ProviderKind::GoogleDrive
            && self
                .client_secret
                .as_deref()
                .is_none_or(|secret| secret.trim().is_empty())
        {
            return Err(Error::invalid(
                "Google needs the client secret from your Desktop app client.",
            ));
        }
        Ok(())
    }

    fn secret(&self) -> Option<&str> {
        self.client_secret
            .as_deref()
            .map(str::trim)
            .filter(|secret| !secret.is_empty())
    }
}

struct Endpoints {
    authorize: String,
    token: String,
}

fn endpoints(kind: ProviderKind, credentials: &AppCredentials) -> Endpoints {
    match kind {
        ProviderKind::GoogleDrive => Endpoints {
            authorize: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token: "https://oauth2.googleapis.com/token".into(),
        },
        ProviderKind::OneDrive => {
            let tenant = credentials
                .tenant
                .as_deref()
                .map(str::trim)
                .filter(|tenant| !tenant.is_empty())
                .unwrap_or("common");
            Endpoints {
                authorize: format!(
                    "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/authorize"
                ),
                token: format!("https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token"),
            }
        }
        ProviderKind::Dropbox => Endpoints {
            authorize: "https://www.dropbox.com/oauth2/authorize".into(),
            token: "https://api.dropboxapi.com/oauth2/token".into(),
        },
    }
}

/// The redirect URI each provider expects for a loopback listener on `port`.
pub fn redirect_uri(kind: ProviderKind, port: u16) -> String {
    match kind {
        ProviderKind::GoogleDrive => format!("http://127.0.0.1:{port}"),
        ProviderKind::OneDrive => format!("http://localhost:{port}"),
        ProviderKind::Dropbox => format!("http://localhost:{port}/"),
    }
}

#[derive(Debug, Clone, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct TokenError {
    #[serde(default)]
    error: String,
    #[serde(default)]
    error_description: String,
}

pub struct TokenSet {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: Option<u64>,
}

/// A sign-in waiting for the browser to come back.
pub struct PendingAuthorization {
    kind: ProviderKind,
    credentials: AppCredentials,
    url: String,
    redirect_uri: String,
    verifier: String,
    state: String,
    listeners: Vec<TcpListener>,
}

impl PendingAuthorization {
    pub async fn begin(kind: ProviderKind, credentials: AppCredentials) -> Result<Self> {
        credentials.validate(kind)?;
        let listeners = bind_loopback(kind).await?;
        let port = listeners[0].local_addr()?.port();
        let redirect_uri = redirect_uri(kind, port);
        let verifier = random_string(64);
        let state = random_string(32);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let mut url = Url::parse(&endpoints(kind, &credentials).authorize)
            .map_err(|error| Error::invalid(error.to_string()))?;
        {
            let mut query = url.query_pairs_mut();
            query
                .append_pair("client_id", credentials.client_id.trim())
                .append_pair("redirect_uri", &redirect_uri)
                .append_pair("response_type", "code")
                .append_pair("code_challenge", &challenge)
                .append_pair("code_challenge_method", "S256")
                .append_pair("state", &state);
            match kind {
                ProviderKind::GoogleDrive => {
                    query
                        .append_pair("scope", "https://www.googleapis.com/auth/drive")
                        .append_pair("access_type", "offline")
                        .append_pair("prompt", "consent");
                }
                ProviderKind::OneDrive => {
                    query
                        .append_pair("scope", "offline_access Files.ReadWrite.All User.Read")
                        .append_pair("prompt", "select_account");
                }
                ProviderKind::Dropbox => {
                    query.append_pair("token_access_type", "offline");
                }
            }
        }
        Ok(Self {
            kind,
            credentials,
            url: url.into(),
            redirect_uri,
            verifier,
            state,
            listeners,
        })
    }

    /// The page to open in the browser.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Waits for the provider to redirect back, then exchanges the code.
    pub async fn finish(self) -> Result<TokenSet> {
        let code = tokio::time::timeout(SIGN_IN_TIMEOUT, self.wait_for_code())
            .await
            .map_err(|_| Error::invalid("Sign-in timed out. Try again."))??;
        let client = reqwest::Client::new();
        let mut form = vec![
            ("grant_type", "authorization_code".to_owned()),
            ("code", code),
            ("redirect_uri", self.redirect_uri.clone()),
            ("client_id", self.credentials.client_id.trim().to_owned()),
            ("code_verifier", self.verifier.clone()),
        ];
        if let Some(secret) = self.credentials.secret() {
            form.push(("client_secret", secret.to_owned()));
        }
        let token = request_token(&client, self.kind, &self.credentials, &form).await?;
        let refresh_token = token.refresh_token.ok_or_else(|| {
            Error::invalid(format!(
                "{} did not grant offline access. Check the app's settings and try again.",
                self.kind.label()
            ))
        })?;
        Ok(TokenSet {
            access_token: token.access_token,
            refresh_token,
            expires_in: token.expires_in,
        })
    }

    async fn wait_for_code(&self) -> Result<String> {
        loop {
            let stream = accept_any(&self.listeners).await?;
            match self.handle_redirect(stream).await? {
                Some(code) => return Ok(code),
                None => continue,
            }
        }
    }

    /// Answers one browser request. Returns the code once the real redirect
    /// arrives; stray requests (a favicon, a preconnect) are ignored.
    async fn handle_redirect(&self, mut stream: TcpStream) -> Result<Option<String>> {
        let request = read_request_head(&mut stream).await?;
        let target = request
            .split_whitespace()
            .nth(1)
            .unwrap_or_default()
            .to_owned();
        let url = Url::parse(&format!("http://localhost{target}"))
            .map_err(|error| Error::invalid(error.to_string()))?;
        let query: HashMap<String, String> = url.query_pairs().into_owned().collect();
        if !query.contains_key("code") && !query.contains_key("error") {
            respond(&mut stream, 404, "Not found").await;
            return Ok(None);
        }
        if query.get("state").map(String::as_str) != Some(self.state.as_str()) {
            respond(
                &mut stream,
                400,
                &page("This sign-in link is out of date. Start again from Kura."),
            )
            .await;
            return Err(Error::invalid("Sign-in state did not match. Try again."));
        }
        if let Some(error) = query.get("error") {
            let detail = query
                .get("error_description")
                .cloned()
                .unwrap_or_else(|| error.clone());
            respond(
                &mut stream,
                200,
                &page("Sign-in was not completed. You can close this tab."),
            )
            .await;
            return Err(Error::invalid(format!("Sign-in failed: {detail}")));
        }
        respond(
            &mut stream,
            200,
            &page("Kura is connected. You can close this tab."),
        )
        .await;
        Ok(query.get("code").cloned())
    }
}

async fn bind_loopback(kind: ProviderKind) -> Result<Vec<TcpListener>> {
    let port = if kind == ProviderKind::Dropbox {
        DROPBOX_REDIRECT_PORT
    } else {
        0
    };
    let v4 = TcpListener::bind(("127.0.0.1", port)).await.map_err(|error| {
        if port != 0 && error.kind() == std::io::ErrorKind::AddrInUse {
            Error::invalid(format!(
                "Port {port} is in use, and Dropbox sign-in needs it. Close the app using it and try again."
            ))
        } else {
            Error::Io(error)
        }
    })?;
    let bound = v4.local_addr()?.port();
    let mut listeners = vec![v4];
    // "localhost" can resolve to ::1 first; listen there too when possible.
    if kind != ProviderKind::GoogleDrive {
        if let Ok(v6) = TcpListener::bind(("::1", bound)).await {
            listeners.push(v6);
        }
    }
    Ok(listeners)
}

async fn accept_any(listeners: &[TcpListener]) -> Result<TcpStream> {
    if listeners.len() == 1 {
        return Ok(listeners[0].accept().await?.0);
    }
    tokio::select! {
        result = listeners[0].accept() => Ok(result?.0),
        result = listeners[1].accept() => Ok(result?.0),
    }
}

async fn read_request_head(stream: &mut TcpStream) -> Result<String> {
    let mut buffer = Vec::with_capacity(2048);
    let mut chunk = [0u8; 1024];
    loop {
        let read = tokio::time::timeout(Duration::from_secs(10), stream.read(&mut chunk))
            .await
            .map_err(|_| Error::invalid("The browser did not finish its request."))??;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") || buffer.len() > 16 * 1024 {
            break;
        }
    }
    Ok(String::from_utf8_lossy(&buffer).into_owned())
}

async fn respond(stream: &mut TcpStream, status: u16, body: &str) {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        _ => "Not Found",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

fn page(message: &str) -> String {
    format!(
        "<!doctype html><meta charset=utf-8><title>Kura</title>\
         <body style=\"font:16px -apple-system,system-ui,sans-serif;display:grid;place-items:center;height:90vh\">\
         <p>{message}</p></body>"
    )
}

fn random_string(length: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

async fn request_token(
    client: &reqwest::Client,
    kind: ProviderKind,
    credentials: &AppCredentials,
    form: &[(&str, String)],
) -> Result<TokenResponse> {
    let response = client
        .post(endpoints(kind, credentials).token)
        .form(form)
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await?;
    if status.is_success() {
        return serde_json::from_str(&body).map_err(Error::from);
    }
    let error: TokenError = serde_json::from_str(&body).unwrap_or(TokenError {
        error: String::new(),
        error_description: body.chars().take(300).collect(),
    });
    Err(Error::Http {
        provider: kind.label(),
        status: status.as_u16(),
        message: if error.error_description.is_empty() {
            error.error
        } else {
            error.error_description
        },
    })
}

/// Hands out access tokens for one remote, refreshing them as needed and
/// saving any refresh token the provider rotates.
pub(crate) struct TokenSource {
    remote: String,
    kind: ProviderKind,
    client: reqwest::Client,
    store: Arc<dyn SecretStore>,
    state: Mutex<TokenState>,
}

struct TokenState {
    secret: RemoteSecret,
    access: Option<(String, Instant)>,
}

impl TokenSource {
    pub(crate) fn new(
        remote: String,
        kind: ProviderKind,
        secret: RemoteSecret,
        store: Arc<dyn SecretStore>,
        initial_access: Option<(String, Option<u64>)>,
    ) -> Self {
        let access = initial_access.map(|(token, expires_in)| {
            (
                token,
                Instant::now() + Duration::from_secs(expires_in.unwrap_or(3000)),
            )
        });
        Self {
            remote,
            kind,
            client: reqwest::Client::new(),
            store,
            state: Mutex::new(TokenState { secret, access }),
        }
    }

    pub(crate) fn remote(&self) -> &str {
        &self.remote
    }

    pub(crate) async fn access_token(&self) -> Result<String> {
        let mut state = self.state.lock().await;
        if let Some((token, expires)) = &state.access {
            if Instant::now() + REFRESH_MARGIN < *expires {
                return Ok(token.clone());
            }
        }
        self.refresh(&mut state).await
    }

    /// Forgets the current access token after the provider rejected it.
    pub(crate) async fn invalidate(&self, rejected: &str) {
        let mut state = self.state.lock().await;
        if state
            .access
            .as_ref()
            .is_some_and(|(token, _)| token == rejected)
        {
            state.access = None;
        }
    }

    async fn refresh(&self, state: &mut TokenState) -> Result<String> {
        let refresh_token = state
            .secret
            .refresh_token
            .clone()
            .filter(|token| !token.is_empty())
            .ok_or_else(|| Error::AuthRequired(self.remote.clone()))?;
        let credentials = state.secret.credentials.clone();
        let mut form = vec![
            ("grant_type", "refresh_token".to_owned()),
            ("refresh_token", refresh_token.clone()),
            ("client_id", credentials.client_id.trim().to_owned()),
        ];
        if let Some(secret) = credentials.secret() {
            form.push(("client_secret", secret.to_owned()));
        }
        let token = match request_token(&self.client, self.kind, &credentials, &form).await {
            Ok(token) => token,
            Err(Error::Http {
                status: 400 | 401, ..
            }) => return Err(Error::AuthRequired(self.remote.clone())),
            Err(error) => return Err(error),
        };
        if let Some(rotated) = token
            .refresh_token
            .clone()
            .filter(|rotated| *rotated != refresh_token)
        {
            state.secret.refresh_token = Some(rotated);
            let store = self.store.clone();
            let remote = self.remote.clone();
            let secret = state.secret.clone();
            tokio::task::spawn_blocking(move || store.save(&remote, &secret))
                .await
                .map_err(|error| Error::Keychain(error.to_string()))??;
        }
        let expires = Instant::now() + Duration::from_secs(token.expires_in.unwrap_or(3600));
        state.access = Some((token.access_token.clone(), expires));
        Ok(token.access_token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn builds_an_authorization_url_with_pkce() {
        let pending = PendingAuthorization::begin(
            ProviderKind::OneDrive,
            AppCredentials {
                client_id: "abc".into(),
                ..AppCredentials::default()
            },
        )
        .await
        .unwrap();
        let url = Url::parse(pending.url()).unwrap();
        let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(query["client_id"], "abc");
        assert_eq!(query["code_challenge_method"], "S256");
        assert!(query["redirect_uri"].starts_with("http://localhost:"));
        assert!(query["scope"].contains("offline_access"));
        assert_eq!(query["state"].len(), 32);
    }

    #[tokio::test]
    async fn loopback_listener_returns_the_code_and_checks_state() {
        let pending = PendingAuthorization::begin(
            ProviderKind::GoogleDrive,
            AppCredentials {
                client_id: "id".into(),
                client_secret: Some("secret".into()),
                tenant: None,
            },
        )
        .await
        .unwrap();
        let port = pending.listeners[0].local_addr().unwrap().port();
        let state = pending.state.clone();
        let waiter = tokio::spawn(async move { pending.wait_for_code().await });

        let mut stray = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        stray
            .write_all(b"GET /favicon.ico HTTP/1.1\r\n\r\n")
            .await
            .unwrap();
        let mut sink = Vec::new();
        stray.read_to_end(&mut sink).await.unwrap();

        let mut browser = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        browser
            .write_all(format!("GET /?code=xyz&state={state} HTTP/1.1\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut reply = String::new();
        browser.read_to_string(&mut reply).await.unwrap();
        assert!(reply.contains("Kura is connected"));
        assert_eq!(waiter.await.unwrap().unwrap(), "xyz");
    }

    #[test]
    fn google_requires_a_client_secret() {
        let credentials = AppCredentials {
            client_id: "id".into(),
            ..AppCredentials::default()
        };
        assert!(credentials.validate(ProviderKind::GoogleDrive).is_err());
        assert!(credentials.validate(ProviderKind::Dropbox).is_ok());
    }
}
