//! Content hashes in the form each provider reports them, so a local file can
//! be compared with its cloud copy without downloading it.
use std::path::Path;

use base64::{engine::general_purpose::STANDARD, Engine};
use md5::{Digest as _, Md5};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tokio::io::AsyncReadExt;

use crate::Result;

mod quickxor;
pub use quickxor::QuickXorHash;

const DROPBOX_BLOCK: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HashKind {
    /// Google Drive `md5Checksum`, lowercase hex.
    Md5,
    /// Dropbox `content_hash`: SHA-256 over the SHA-256 of each 4 MiB block, hex.
    DropboxContent,
    /// OneDrive `quickXorHash`, base64.
    QuickXor,
}

pub struct ContentHasher {
    state: State,
}

enum State {
    Md5(Md5),
    Dropbox {
        overall: Sha256,
        block: Sha256,
        block_len: usize,
    },
    QuickXor(Box<QuickXorHash>),
}

impl ContentHasher {
    pub fn new(kind: HashKind) -> Self {
        let state = match kind {
            HashKind::Md5 => State::Md5(Md5::new()),
            HashKind::DropboxContent => State::Dropbox {
                overall: Sha256::new(),
                block: Sha256::new(),
                block_len: 0,
            },
            HashKind::QuickXor => State::QuickXor(Box::default()),
        };
        Self { state }
    }

    pub fn update(&mut self, mut bytes: &[u8]) {
        match &mut self.state {
            State::Md5(hash) => hash.update(bytes),
            State::QuickXor(hash) => hash.update(bytes),
            State::Dropbox {
                overall,
                block,
                block_len,
            } => {
                while !bytes.is_empty() {
                    let take = (DROPBOX_BLOCK - *block_len).min(bytes.len());
                    block.update(&bytes[..take]);
                    *block_len += take;
                    bytes = &bytes[take..];
                    if *block_len == DROPBOX_BLOCK {
                        let finished = std::mem::take(block);
                        overall.update(finished.finalize());
                        *block_len = 0;
                    }
                }
            }
        }
    }

    pub fn finish(self) -> String {
        match self.state {
            State::Md5(hash) => hex::encode(hash.finalize()),
            State::QuickXor(hash) => STANDARD.encode(hash.finalize()),
            State::Dropbox {
                mut overall,
                block,
                block_len,
            } => {
                if block_len > 0 {
                    overall.update(block.finalize());
                }
                hex::encode(overall.finalize())
            }
        }
    }
}

/// Hashes a file the way `kind` expects, reading it in large chunks.
pub async fn hash_file(path: &Path, kind: HashKind) -> Result<String> {
    let mut file = tokio::fs::File::open(path).await?;
    let mut hasher = ContentHasher::new(kind);
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finish())
}

/// Provider hashes differ only in letter case at most (Drive and Dropbox use hex).
pub fn same_hash(left: &str, right: &str) -> bool {
    !left.is_empty() && left.eq_ignore_ascii_case(right)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(kind: HashKind, data: &[u8]) -> String {
        let mut hasher = ContentHasher::new(kind);
        hasher.update(data);
        hasher.finish()
    }

    #[test]
    fn md5_matches_known_digest() {
        assert_eq!(
            digest(HashKind::Md5, b"hello"),
            "5d41402abc4b2a76b9719d911017c592"
        );
    }

    #[test]
    fn dropbox_hash_of_empty_file_is_sha256_of_nothing() {
        assert_eq!(
            digest(HashKind::DropboxContent, b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn dropbox_hash_splits_into_four_mib_blocks() {
        let data = vec![7u8; DROPBOX_BLOCK + 10];
        let mut expected = Sha256::new();
        expected.update(Sha256::digest(&data[..DROPBOX_BLOCK]));
        expected.update(Sha256::digest(&data[DROPBOX_BLOCK..]));
        let expected = hex::encode(expected.finalize());
        assert_eq!(digest(HashKind::DropboxContent, &data), expected);

        let mut chunked = ContentHasher::new(HashKind::DropboxContent);
        for part in data.chunks(999_999) {
            chunked.update(part);
        }
        assert_eq!(chunked.finish(), expected);
    }

    #[tokio::test]
    async fn hashes_files_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        std::fs::write(&path, b"hello").unwrap();
        assert_eq!(
            hash_file(&path, HashKind::Md5).await.unwrap(),
            "5d41402abc4b2a76b9719d911017c592"
        );
    }
}
