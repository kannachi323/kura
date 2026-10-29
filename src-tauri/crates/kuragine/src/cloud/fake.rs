//! An in-memory drive with Google Drive's shape (IDs, MD5, versions) so the
//! engine can be tested without a network.
use std::{collections::HashMap, path::Path, sync::Mutex};

use super::{Account, Downloaded, Quota, RemoteItem, TransferObserver, UploadTarget, ROOT_ID};
use crate::{
    hash::{ContentHasher, HashKind},
    Error, Result,
};

#[derive(Clone)]
struct Entry {
    name: String,
    parent: String,
    is_dir: bool,
    data: Vec<u8>,
    version: u64,
    trashed: bool,
}

#[derive(Default)]
pub struct FakeDrive {
    state: Mutex<(u64, HashMap<String, Entry>)>,
}

fn md5(data: &[u8]) -> String {
    let mut hasher = ContentHasher::new(HashKind::Md5);
    hasher.update(data);
    hasher.finish()
}

impl FakeDrive {
    pub fn new() -> Self {
        let drive = Self::default();
        drive.state.lock().unwrap().1.insert(
            ROOT_ID.into(),
            Entry {
                name: String::new(),
                parent: String::new(),
                is_dir: true,
                data: Vec::new(),
                version: 1,
                trashed: false,
            },
        );
        drive
    }

    fn insert(&self, parent: &str, name: &str, is_dir: bool, data: &[u8]) -> String {
        let mut state = self.state.lock().unwrap();
        state.0 += 1;
        let id = format!("f{}", state.0);
        state.1.insert(
            id.clone(),
            Entry {
                name: name.into(),
                parent: parent.into(),
                is_dir,
                data: data.to_vec(),
                version: 1,
                trashed: false,
            },
        );
        id
    }

    pub fn add_file(&self, parent: &str, name: &str, data: &[u8]) -> String {
        self.insert(parent, name, false, data)
    }

    pub fn add_folder(&self, parent: &str, name: &str) -> String {
        self.insert(parent, name, true, &[])
    }

    /// Changes a file as another device would.
    pub fn set_content(&self, id: &str, data: &[u8]) {
        let mut state = self.state.lock().unwrap();
        let entry = state.1.get_mut(id).unwrap();
        entry.data = data.to_vec();
        entry.version += 1;
    }

    pub fn content(&self, id: &str) -> Vec<u8> {
        self.state.lock().unwrap().1[id].data.clone()
    }

    pub fn find(&self, parent: &str, name: &str) -> Option<String> {
        self.state
            .lock()
            .unwrap()
            .1
            .iter()
            .find(|(_, entry)| entry.parent == parent && entry.name == name && !entry.trashed)
            .map(|(id, _)| id.clone())
    }

    pub fn is_trashed(&self, id: &str) -> bool {
        self.state.lock().unwrap().1[id].trashed
    }

    fn item(id: &str, entry: &Entry) -> RemoteItem {
        RemoteItem {
            id: id.into(),
            name: entry.name.clone(),
            is_dir: entry.is_dir,
            size: entry.data.len() as u64,
            modified_ms: Some(entry.version as i64 * 1000),
            revision: Some(entry.version.to_string()),
            hash: (!entry.is_dir).then(|| md5(&entry.data)),
            ..RemoteItem::default()
        }
    }

    fn get(&self, id: &str) -> Result<RemoteItem> {
        let state = self.state.lock().unwrap();
        match state.1.get(id) {
            Some(entry) if !entry.trashed => Ok(Self::item(id, entry)),
            _ => Err(Error::NotFound(id.into())),
        }
    }

    pub async fn account(&self) -> Result<Account> {
        Ok(Account {
            email: "me@example.com".into(),
            display_name: "Me".into(),
            drive_type: String::new(),
        })
    }

    pub async fn quota(&self) -> Result<Quota> {
        Ok(Quota::default())
    }

    pub async fn root(&self) -> Result<RemoteItem> {
        self.get(ROOT_ID)
    }

    pub async fn list(&self, folder: &RemoteItem) -> Result<Vec<RemoteItem>> {
        let state = self.state.lock().unwrap();
        Ok(state
            .1
            .iter()
            .filter(|(_, entry)| entry.parent == folder.id && !entry.trashed)
            .map(|(id, entry)| Self::item(id, entry))
            .collect())
    }

    pub async fn stat(&self, item: &RemoteItem) -> Result<RemoteItem> {
        self.get(&item.id)
    }

    pub async fn download(
        &self,
        item: &RemoteItem,
        destination: &Path,
        observer: &dyn TransferObserver,
    ) -> Result<Downloaded> {
        let data = {
            let state = self.state.lock().unwrap();
            state
                .1
                .get(&item.id)
                .ok_or_else(|| Error::NotFound(item.id.clone()))?
                .data
                .clone()
        };
        tokio::fs::write(destination, &data).await?;
        observer.progress(data.len() as u64, data.len() as u64);
        Ok(Downloaded {
            bytes: data.len() as u64,
            hash: md5(&data),
        })
    }

    pub async fn upload(
        &self,
        target: UploadTarget<'_>,
        source: &Path,
        _observer: &dyn TransferObserver,
    ) -> Result<RemoteItem> {
        let data = tokio::fs::read(source).await?;
        if let Some((existing, expected)) = target.existing {
            let mut state = self.state.lock().unwrap();
            let entry = state
                .1
                .get_mut(&existing.id)
                .ok_or_else(|| Error::NotFound(existing.id.clone()))?;
            if expected.is_some_and(|expected| expected != entry.version.to_string()) {
                return Err(Error::Conflict(entry.name.clone()));
            }
            entry.data = data;
            entry.version += 1;
            return Ok(Self::item(&existing.id, entry));
        }
        if self.find(&target.parent.id, target.name).is_some() {
            return Err(Error::Conflict(target.name.into()));
        }
        let id = self.add_file(&target.parent.id, target.name, &data);
        self.get(&id)
    }

    pub async fn create_folder(&self, parent: &RemoteItem, name: &str) -> Result<RemoteItem> {
        if self.find(&parent.id, name).is_some() {
            return Err(Error::Conflict(name.into()));
        }
        let id = self.add_folder(&parent.id, name);
        self.get(&id)
    }

    pub async fn rename_move(
        &self,
        item: &RemoteItem,
        new_name: &str,
        new_parent: Option<(&RemoteItem, &RemoteItem)>,
    ) -> Result<RemoteItem> {
        let mut state = self.state.lock().unwrap();
        let entry = state
            .1
            .get_mut(&item.id)
            .ok_or_else(|| Error::NotFound(item.id.clone()))?;
        entry.name = new_name.into();
        if let Some((to, _)) = new_parent {
            entry.parent = to.id.clone();
        }
        entry.version += 1;
        Ok(Self::item(&item.id, entry))
    }

    pub async fn copy_file(
        &self,
        item: &RemoteItem,
        parent: &RemoteItem,
        name: &str,
    ) -> Result<RemoteItem> {
        let data = self.content(&item.id);
        let id = self.add_file(&parent.id, name, &data);
        self.get(&id)
    }

    pub async fn trash(&self, item: &RemoteItem) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        let entry = state
            .1
            .get_mut(&item.id)
            .ok_or_else(|| Error::NotFound(item.id.clone()))?;
        entry.trashed = true;
        Ok(())
    }
}
