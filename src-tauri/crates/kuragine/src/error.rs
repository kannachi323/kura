use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error("{0} was not found.")]
    NotFound(String),
    /// The cloud copy changed since Kura last saw it.
    #[error("{0} changed in the cloud since it was downloaded.")]
    Conflict(String),
    /// The remote's sign-in is missing or was revoked.
    #[error("Sign in to {0} again.")]
    AuthRequired(String),
    #[error("Canceled")]
    Canceled,
    #[error("{provider} returned {status}: {message}")]
    Http {
        provider: &'static str,
        status: u16,
        message: String,
    },
    #[error("Network error: {0}")]
    Network(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("Kuragine database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("Keychain error: {0}")]
    Keychain(String),
    #[error("Unexpected response: {0}")]
    Decode(String),
}

impl Error {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid(message.into())
    }

    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound(_) | Self::Http { status: 404, .. })
            || matches!(self, Self::Io(error) if error.kind() == std::io::ErrorKind::NotFound)
    }
}

impl From<reqwest::Error> for Error {
    fn from(value: reqwest::Error) -> Self {
        Self::Network(value.without_url().to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::Decode(value.to_string())
    }
}

impl From<keyring::Error> for Error {
    fn from(value: keyring::Error) -> Self {
        Self::Keychain(value.to_string())
    }
}

impl Serialize for Error {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
