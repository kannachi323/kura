//! Google Drive, OneDrive and Dropbox behind one interface.
use std::{path::Path, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::{hash::HashKind, Result};

pub mod auth;
mod dropbox;
#[cfg(any(test, feature = "test-support"))]
pub mod fake;
mod gdrive;
mod http;
mod onedrive;
pub mod secrets;

pub use auth::AppCredentials;
use http::Api;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    #[serde(rename = "gdrive")]
    GoogleDrive,
    #[serde(rename = "onedrive")]
    OneDrive,
    #[serde(rename = "dropbox")]
    Dropbox,
}

impl ProviderKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::GoogleDrive => "gdrive",
            Self::OneDrive => "onedrive",
            Self::Dropbox => "dropbox",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "gdrive" => Some(Self::GoogleDrive),
            "onedrive" => Some(Self::OneDrive),
            "dropbox" => Some(Self::Dropbox),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::GoogleDrive => "Google Drive",
            Self::OneDrive => "OneDrive",
            Self::Dropbox => "Dropbox",
        }
    }

    pub fn hash_kind(self) -> HashKind {
        match self {
            Self::GoogleDrive => HashKind::Md5,
            Self::OneDrive => HashKind::QuickXor,
            Self::Dropbox => HashKind::DropboxContent,
        }
    }
}

/// The ID every provider uses here for the top of the drive.
pub const ROOT_ID: &str = "root";

/// One file or folder as a provider describes it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteItem {
    pub id: String,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified_ms: Option<i64>,
    pub mime_type: Option<String>,
    /// Changes whenever the item changes: Drive `version`, OneDrive `eTag`,
    /// Dropbox `rev`.
    pub revision: Option<String>,
    /// Content hash of the provider's `HashKind`; absent for folders and
    /// Google-native documents.
    pub hash: Option<String>,
    /// Dropbox's `path_display`; uploads and new folders are addressed by path.
    pub provider_path: Option<String>,
    /// A Drive shortcut's target, used for listing and downloading.
    pub target_id: Option<String>,
    /// For Google Docs, Sheets, Slides and Drawings: the format Kura exports.
    pub export_mime: Option<String>,
}

impl RemoteItem {
    /// The ID that holds the content (a shortcut's target, otherwise the item).
    pub fn content_id(&self) -> &str {
        self.target_id.as_deref().unwrap_or(&self.id)
    }

    /// The extension Kura adds to an exported Google document's name.
    pub fn export_extension(&self) -> Option<&'static str> {
        gdrive::export_extension(self.export_mime.as_deref()?)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub email: String,
    pub display_name: String,
    /// OneDrive's `driveType` ("personal", "business"); empty elsewhere.
    pub drive_type: String,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Quota {
    pub used_bytes: u64,
    /// Zero when the provider reports no limit.
    pub total_bytes: u64,
}

pub struct Downloaded {
    pub bytes: u64,
    /// The content hash of what was written, in the provider's `HashKind`.
    pub hash: String,
}

/// What an upload replaces, if anything.
#[derive(Debug, Clone)]
pub struct UploadTarget<'a> {
    pub parent: &'a RemoteItem,
    pub name: &'a str,
    /// The existing item and the revision Kura expects it to still have.
    pub existing: Option<(&'a RemoteItem, Option<&'a str>)>,
}

/// Progress and cancellation for one transfer.
pub trait TransferObserver: Send + Sync {
    fn progress(&self, done: u64, total: u64);
    fn is_canceled(&self) -> bool;
}

pub struct NoProgress;

impl TransferObserver for NoProgress {
    fn progress(&self, _done: u64, _total: u64) {}
    fn is_canceled(&self) -> bool {
        false
    }
}

/// Milliseconds since the epoch for an RFC 3339 timestamp.
pub(crate) fn parse_time_ms(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|time| time.timestamp_millis())
}

pub(crate) fn ensure_running(observer: &dyn TransferObserver) -> Result<()> {
    if observer.is_canceled() {
        Err(crate::Error::Canceled)
    } else {
        Ok(())
    }
}

/// A signed-in remote.
#[derive(Clone)]
pub struct Provider {
    kind: ProviderKind,
    backend: Backend,
}

#[derive(Clone)]
enum Backend {
    Http(Arc<Api>),
    #[cfg(any(test, feature = "test-support"))]
    Fake(Arc<fake::FakeDrive>),
}

macro_rules! dispatch {
    ($self:ident, $method:ident ( $($arg:expr),* )) => {
        match &$self.backend {
            Backend::Http(api) => match $self.kind {
                ProviderKind::GoogleDrive => gdrive::$method(api, $($arg),*).await,
                ProviderKind::OneDrive => onedrive::$method(api, $($arg),*).await,
                ProviderKind::Dropbox => dropbox::$method(api, $($arg),*).await,
            },
            #[cfg(any(test, feature = "test-support"))]
            Backend::Fake(fake) => fake.$method($($arg),*).await,
        }
    };
}

impl Provider {
    pub(crate) fn new(kind: ProviderKind, tokens: auth::TokenSource) -> Self {
        Self {
            kind,
            backend: Backend::Http(Arc::new(Api::new(kind, tokens))),
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn fake(drive: Arc<fake::FakeDrive>) -> Self {
        Self {
            kind: ProviderKind::GoogleDrive,
            backend: Backend::Fake(drive),
        }
    }

    pub fn kind(&self) -> ProviderKind {
        self.kind
    }

    pub async fn account(&self) -> Result<Account> {
        dispatch!(self, account())
    }

    pub async fn quota(&self) -> Result<Quota> {
        dispatch!(self, quota())
    }

    pub async fn root(&self) -> Result<RemoteItem> {
        dispatch!(self, root())
    }

    pub async fn list(&self, folder: &RemoteItem) -> Result<Vec<RemoteItem>> {
        dispatch!(self, list(folder))
    }

    pub async fn stat(&self, item: &RemoteItem) -> Result<RemoteItem> {
        dispatch!(self, stat(item))
    }

    /// Streams the item's content (or a Google export) into `destination`.
    pub async fn download(
        &self,
        item: &RemoteItem,
        destination: &Path,
        observer: &dyn TransferObserver,
    ) -> Result<Downloaded> {
        dispatch!(self, download(item, destination, observer))
    }

    pub async fn upload(
        &self,
        target: UploadTarget<'_>,
        source: &Path,
        observer: &dyn TransferObserver,
    ) -> Result<RemoteItem> {
        dispatch!(self, upload(target, source, observer))
    }

    pub async fn create_folder(&self, parent: &RemoteItem, name: &str) -> Result<RemoteItem> {
        dispatch!(self, create_folder(parent, name))
    }

    /// Renames and/or moves an item. `new_parent` is the destination folder
    /// when it changes, together with the folder it leaves.
    pub async fn rename_move(
        &self,
        item: &RemoteItem,
        new_name: &str,
        new_parent: Option<(&RemoteItem, &RemoteItem)>,
    ) -> Result<RemoteItem> {
        dispatch!(self, rename_move(item, new_name, new_parent))
    }

    /// Server-side copy of a file.
    pub async fn copy_file(
        &self,
        item: &RemoteItem,
        parent: &RemoteItem,
        name: &str,
    ) -> Result<RemoteItem> {
        dispatch!(self, copy_file(item, parent, name))
    }

    /// Moves an item to the provider's trash or recycle bin.
    pub async fn trash(&self, item: &RemoteItem) -> Result<()> {
        dispatch!(self, trash(item))
    }
}
