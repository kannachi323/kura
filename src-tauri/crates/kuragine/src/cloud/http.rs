//! Authenticated requests with retries for every provider.
use std::{path::Path, time::Duration};

use bytes::Bytes;
use rand::Rng;
use reqwest::{header, RequestBuilder, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::{auth::TokenSource, ensure_running, Downloaded, ProviderKind, TransferObserver};
use crate::{hash::ContentHasher, Error, Result};

const MAX_ATTEMPTS: u32 = 7;
const MAX_BACKOFF: Duration = Duration::from_secs(32);
const MAX_RETRY_AFTER: Duration = Duration::from_secs(120);

pub(crate) struct Api {
    pub(crate) kind: ProviderKind,
    pub(crate) client: reqwest::Client,
    /// For upload sessions, whose 308 replies must not be followed.
    pub(crate) no_redirect: reqwest::Client,
    remote: String,
    tokens: TokenSource,
}

impl Api {
    pub(crate) fn new(kind: ProviderKind, tokens: TokenSource) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .read_timeout(Duration::from_secs(90))
            .user_agent(concat!("Kura/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_default();
        let no_redirect = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .read_timeout(Duration::from_secs(120))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("Kura/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_default();
        Self {
            kind,
            client,
            no_redirect,
            remote: tokens.remote().to_owned(),
            tokens,
        }
    }

    /// Sends a request built by `build` with the remote's access token,
    /// retrying throttling, server errors and dropped connections. Returns the
    /// response for any 2xx or 3xx status; other statuses become errors.
    pub(crate) async fn send<F>(&self, build: F) -> Result<Response>
    where
        F: Fn(&reqwest::Client) -> RequestBuilder + Sync,
    {
        self.send_inner(&build, true).await
    }

    /// Like `send`, without the access token (OneDrive upload URLs reject it).
    pub(crate) async fn send_anonymous<F>(&self, build: F) -> Result<Response>
    where
        F: Fn(&reqwest::Client) -> RequestBuilder + Sync,
    {
        self.send_inner(&build, false).await
    }

    pub(crate) async fn json<T, F>(&self, build: F) -> Result<T>
    where
        T: DeserializeOwned,
        F: Fn(&reqwest::Client) -> RequestBuilder + Sync,
    {
        let response = self.send(build).await?;
        let body = response.bytes().await?;
        serde_json::from_slice(&body)
            .map_err(|error| Error::Decode(format!("{} response: {error}", self.kind.label())))
    }

    async fn send_inner(
        &self,
        build: &(dyn Fn(&reqwest::Client) -> RequestBuilder + Sync),
        authenticate: bool,
    ) -> Result<Response> {
        let mut refreshed = false;
        let mut attempt = 0;
        loop {
            attempt += 1;
            let mut request = build(&self.client);
            let mut token = String::new();
            if authenticate {
                token = self.tokens.access_token().await?;
                request = request.bearer_auth(&token);
            }
            let response = match request.send().await {
                Ok(response) => response,
                Err(error) if attempt < MAX_ATTEMPTS && is_transient(&error) => {
                    tokio::time::sleep(backoff(attempt)).await;
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            let status = response.status();
            if status.is_success() || status.is_redirection() {
                return Ok(response);
            }
            if status == StatusCode::UNAUTHORIZED && authenticate && !refreshed {
                refreshed = true;
                self.tokens.invalidate(&token).await;
                continue;
            }
            let retry_after = retry_after(&response);
            let body = response.text().await.unwrap_or_default();
            let throttled = status == StatusCode::TOO_MANY_REQUESTS
                || status.is_server_error()
                || (status == StatusCode::FORBIDDEN && is_drive_rate_limit(&body));
            if throttled && attempt < MAX_ATTEMPTS {
                tokio::time::sleep(retry_after.unwrap_or_else(|| backoff(attempt))).await;
                continue;
            }
            return Err(self.status_error(status, &body, authenticate));
        }
    }

    fn status_error(&self, status: StatusCode, body: &str, authenticated: bool) -> Error {
        let message = provider_message(body);
        match status {
            StatusCode::UNAUTHORIZED if authenticated => Error::AuthRequired(self.remote.clone()),
            StatusCode::NOT_FOUND => Error::NotFound(if message.is_empty() {
                "The item".to_owned()
            } else {
                message
            }),
            StatusCode::PRECONDITION_FAILED => Error::Conflict(message),
            _ => Error::Http {
                provider: self.kind.label(),
                status: status.as_u16(),
                message,
            },
        }
    }
}

/// Streams a download into `destination`, hashing it as the provider does.
pub(crate) async fn write_body(
    kind: ProviderKind,
    mut response: Response,
    destination: &Path,
    observer: &dyn TransferObserver,
) -> Result<Downloaded> {
    let total = response.content_length().unwrap_or(0);
    let mut file = tokio::fs::File::create(destination).await?;
    let mut hasher = ContentHasher::new(kind.hash_kind());
    let mut bytes = 0u64;
    while let Some(chunk) = response.chunk().await? {
        ensure_running(observer)?;
        file.write_all(&chunk).await?;
        hasher.update(&chunk);
        bytes += chunk.len() as u64;
        observer.progress(bytes, total.max(bytes));
    }
    file.flush().await?;
    file.sync_all().await?;
    Ok(Downloaded {
        bytes,
        hash: hasher.finish(),
    })
}

/// Reads the next `len` bytes of an upload source.
pub(crate) async fn read_chunk(file: &mut tokio::fs::File, len: usize) -> Result<Bytes> {
    let mut buffer = vec![0u8; len];
    file.read_exact(&mut buffer).await?;
    Ok(Bytes::from(buffer))
}

fn is_transient(error: &reqwest::Error) -> bool {
    error.is_timeout() || error.is_connect() || error.is_request()
}

fn backoff(attempt: u32) -> Duration {
    let base = Duration::from_millis(500) * 2u32.saturating_pow(attempt.saturating_sub(1));
    let jitter = Duration::from_millis(rand::thread_rng().gen_range(0..500));
    base.min(MAX_BACKOFF) + jitter
}

fn retry_after(response: &Response) -> Option<Duration> {
    let seconds: u64 = response
        .headers()
        .get(header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()?;
    Some(Duration::from_secs(seconds).min(MAX_RETRY_AFTER))
}

fn is_drive_rate_limit(body: &str) -> bool {
    body.contains("rateLimitExceeded") || body.contains("userRateLimitExceeded")
}

/// Pulls the human message out of each provider's error body.
pub(crate) fn provider_message(body: &str) -> String {
    if let Ok(value) = serde_json::from_str::<Value>(body) {
        let candidates = [
            value.pointer("/error/message"),
            value.pointer("/error_summary"),
            value.pointer("/error_description"),
            value.pointer("/error"),
        ];
        for candidate in candidates.into_iter().flatten() {
            if let Some(text) = candidate.as_str() {
                return text.trim().to_owned();
            }
        }
    }
    body.chars().take(300).collect::<String>().trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_messages_from_each_provider() {
        assert_eq!(
            provider_message(r#"{"error":{"code":403,"message":"Rate limited"}}"#),
            "Rate limited"
        );
        assert_eq!(
            provider_message(r#"{"error_summary":"path/not_found/..","error":{".tag":"path"}}"#),
            "path/not_found/.."
        );
        assert_eq!(provider_message("plain"), "plain");
    }

    #[test]
    fn backoff_grows_and_is_capped() {
        assert!(backoff(1) < Duration::from_secs(1) + Duration::from_millis(1));
        assert!(backoff(20) <= MAX_BACKOFF + Duration::from_millis(500));
    }
}
