//! Where each remote's app credentials and refresh token live: the OS keychain
//! in the app, memory in tests.
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};

use super::auth::AppCredentials;
use crate::{Error, Result};

const KEYCHAIN_SERVICE: &str = "dev.kura.kuragine";

/// Everything secret about one remote, stored as one keychain item so signing
/// in asks for keychain access at most once.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSecret {
    pub credentials: AppCredentials,
    #[serde(default)]
    pub refresh_token: Option<String>,
}

pub trait SecretStore: Send + Sync {
    fn load(&self, remote: &str) -> Result<Option<RemoteSecret>>;
    fn save(&self, remote: &str, secret: &RemoteSecret) -> Result<()>;
    fn delete(&self, remote: &str) -> Result<()>;
}

pub struct KeychainStore;

impl SecretStore for KeychainStore {
    fn load(&self, remote: &str) -> Result<Option<RemoteSecret>> {
        let entry = keyring::Entry::new(KEYCHAIN_SERVICE, remote)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(serde_json::from_str(&value)?)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn save(&self, remote: &str, secret: &RemoteSecret) -> Result<()> {
        let entry = keyring::Entry::new(KEYCHAIN_SERVICE, remote)?;
        entry.set_password(&serde_json::to_string(secret)?)?;
        Ok(())
    }

    fn delete(&self, remote: &str) -> Result<()> {
        let entry = keyring::Entry::new(KEYCHAIN_SERVICE, remote)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(Error::from(error)),
        }
    }
}

#[derive(Default, Clone)]
pub struct MemoryStore {
    secrets: Arc<Mutex<HashMap<String, RemoteSecret>>>,
}

impl SecretStore for MemoryStore {
    fn load(&self, remote: &str) -> Result<Option<RemoteSecret>> {
        Ok(self.secrets.lock().unwrap().get(remote).cloned())
    }

    fn save(&self, remote: &str, secret: &RemoteSecret) -> Result<()> {
        self.secrets
            .lock()
            .unwrap()
            .insert(remote.to_owned(), secret.clone());
        Ok(())
    }

    fn delete(&self, remote: &str) -> Result<()> {
        self.secrets.lock().unwrap().remove(remote);
        Ok(())
    }
}
