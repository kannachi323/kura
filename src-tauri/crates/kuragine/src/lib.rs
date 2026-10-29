//! Kuragine: Kura's file engine.
//!
//! It talks to Google Drive, OneDrive and Dropbox with the person's own OAuth
//! apps, keeps downloaded files under `~/.kura/providers/<remote>/`, and turns
//! local edits into uploads that stop at conflicts. See `docs/kuragine.md`.
pub mod cloud;
mod engine;
mod error;
pub mod hash;
pub mod names;
mod store;
mod watch;

pub use cloud::{
    secrets::{KeychainStore, MemoryStore, SecretStore},
    AppCredentials, NoProgress, ProviderKind, Quota, TransferObserver,
};
pub use engine::{
    CloudEntry, CloudPath, ConflictChoice, ConflictInfo, CopyState, EngineEvent, Kuragine,
    KuragineConfig, RemoteInfo, UploadOutcome,
};
pub use error::{Error, Result};
