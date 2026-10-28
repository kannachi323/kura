//! Kuragine's index in SQLite: remotes, the cloud items Kura has listed,
//! the files it downloaded (with their base for change detection) and
//! uploads stopped by a conflict.
use std::{path::Path, sync::Mutex, time::Duration};

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::{
    cloud::{ProviderKind, RemoteItem},
    names, Error, Result,
};

const SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RemoteRow {
    pub name: String,
    pub kind: ProviderKind,
    pub email: String,
    pub display_name: String,
    pub drive_type: String,
    pub created_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ItemRow {
    pub path: String,
    pub parent_path: String,
    pub item: RemoteItem,
    /// For folders: when the children were last listed; 0 if never.
    pub listed_ms: i64,
}

/// A downloaded file and the state Kuragine left it in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CopyRow {
    pub path: String,
    pub id: String,
    /// Content hash (provider's kind) of the version the local file started from.
    pub base_hash: String,
    pub base_revision: String,
    pub local_size: u64,
    pub local_mtime_ns: i64,
    pub downloaded_ms: i64,
    pub read_only: bool,
    pub checked_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConflictRow {
    pub remote: String,
    pub path: String,
    pub id: String,
    pub remote_revision: String,
    pub remote_hash: String,
    /// Where the cloud version was downloaded for comparison.
    pub remote_copy: String,
    pub detected_ms: i64,
}

pub(crate) struct Store {
    conn: Mutex<Connection>,
}

/// SQL matching `path` and everything under it for one remote (`?1` remote,
/// `?2` path).
const SUBTREE: &str =
    "remote = ?1 AND (?2 = '/' OR path = ?2 OR (path > ?2 || '/' AND path < ?2 || '0'))";

impl Store {
    pub(crate) fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    pub(crate) fn in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.busy_timeout(Duration::from_secs(10))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(Error::invalid(
                "kuragine.db was written by a newer Kura. Update Kura to use cloud files.",
            ));
        }
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS remotes (
                name TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                email TEXT NOT NULL DEFAULT '',
                display_name TEXT NOT NULL DEFAULT '',
                drive_type TEXT NOT NULL DEFAULT '',
                created_ms INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS items (
                remote TEXT NOT NULL,
                path TEXT NOT NULL,
                parent_path TEXT NOT NULL,
                id TEXT NOT NULL,
                name TEXT NOT NULL,
                is_dir INTEGER NOT NULL,
                size INTEGER NOT NULL DEFAULT 0,
                modified_ms INTEGER,
                mime_type TEXT,
                revision TEXT,
                hash TEXT,
                provider_path TEXT,
                target_id TEXT,
                export_mime TEXT,
                listed_ms INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (remote, path)
            );
            CREATE INDEX IF NOT EXISTS items_parent ON items(remote, parent_path);
            CREATE TABLE IF NOT EXISTS copies (
                remote TEXT NOT NULL,
                path TEXT NOT NULL,
                id TEXT NOT NULL,
                base_hash TEXT NOT NULL,
                base_revision TEXT NOT NULL DEFAULT '',
                local_size INTEGER NOT NULL,
                local_mtime_ns INTEGER NOT NULL,
                downloaded_ms INTEGER NOT NULL,
                read_only INTEGER NOT NULL DEFAULT 0,
                checked_ms INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (remote, path)
            );
            CREATE TABLE IF NOT EXISTS conflicts (
                remote TEXT NOT NULL,
                path TEXT NOT NULL,
                id TEXT NOT NULL,
                remote_revision TEXT NOT NULL DEFAULT '',
                remote_hash TEXT NOT NULL DEFAULT '',
                remote_copy TEXT NOT NULL,
                detected_ms INTEGER NOT NULL,
                PRIMARY KEY (remote, path)
            );",
        )?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn with<T>(&self, run: impl FnOnce(&mut Connection) -> rusqlite::Result<T>) -> Result<T> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|_| Error::invalid("Kuragine's database lock was poisoned."))?;
        Ok(run(&mut conn)?)
    }

    // Remotes

    pub(crate) fn remotes(&self) -> Result<Vec<RemoteRow>> {
        self.with(|conn| {
            let mut statement = conn.prepare(
                "SELECT name, kind, email, display_name, drive_type, created_ms
                 FROM remotes ORDER BY name COLLATE NOCASE",
            )?;
            let rows = statement.query_map([], read_remote)?;
            rows.filter_map(|row| row.transpose()).collect()
        })
    }

    pub(crate) fn remote(&self, name: &str) -> Result<Option<RemoteRow>> {
        self.with(|conn| {
            conn.query_row(
                "SELECT name, kind, email, display_name, drive_type, created_ms
                 FROM remotes WHERE name = ?1 COLLATE NOCASE",
                [name],
                read_remote,
            )
            .optional()
            .map(Option::flatten)
        })
    }

    pub(crate) fn put_remote(&self, row: &RemoteRow) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "INSERT INTO remotes (name, kind, email, display_name, drive_type, created_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(name) DO UPDATE SET kind = excluded.kind, email = excluded.email,
                   display_name = excluded.display_name, drive_type = excluded.drive_type",
                params![
                    row.name,
                    row.kind.id(),
                    row.email,
                    row.display_name,
                    row.drive_type,
                    row.created_ms
                ],
            )
            .map(|_| ())
        })
    }

    pub(crate) fn delete_remote(&self, name: &str) -> Result<()> {
        self.with(|conn| {
            let tx = conn.transaction()?;
            for table in ["remotes", "items", "copies", "conflicts"] {
                let column = if table == "remotes" { "name" } else { "remote" };
                tx.execute(&format!("DELETE FROM {table} WHERE {column} = ?1"), [name])?;
            }
            tx.commit()
        })
    }

    /// Forgets every listed item of a remote (after its account changed).
    pub(crate) fn clear_items(&self, remote: &str) -> Result<()> {
        self.with(|conn| {
            conn.execute("DELETE FROM items WHERE remote = ?1", [remote])
                .map(|_| ())
        })
    }

    // Items

    pub(crate) fn item(&self, remote: &str, path: &str) -> Result<Option<ItemRow>> {
        self.with(|conn| {
            conn.query_row(
                &format!("SELECT {ITEM_COLUMNS} FROM items WHERE remote = ?1 AND path = ?2"),
                params![remote, path],
                read_item,
            )
            .optional()
        })
    }

    pub(crate) fn children(&self, remote: &str, path: &str) -> Result<Vec<ItemRow>> {
        self.with(|conn| {
            let mut statement = conn.prepare(&format!(
                "SELECT {ITEM_COLUMNS} FROM items WHERE remote = ?1 AND parent_path = ?2 AND path <> '/'"
            ))?;
            let rows = statement.query_map(params![remote, path], read_item)?;
            rows.collect()
        })
    }

    pub(crate) fn put_item(&self, remote: &str, row: &ItemRow) -> Result<()> {
        self.with(|conn| insert_item(conn, remote, row))
    }

    /// Replaces a folder's children with a fresh listing. Children that are
    /// gone take their subtrees with them; children still present keep the
    /// `listed_ms` of their own subtree. Returns the removed child paths.
    pub(crate) fn replace_children(
        &self,
        remote: &str,
        parent: &str,
        children: &[ItemRow],
        listed_ms: i64,
    ) -> Result<Vec<String>> {
        self.with(|conn| {
            let tx = conn.transaction()?;
            let existing: Vec<String> = {
                let mut statement = tx.prepare(
                    "SELECT path FROM items WHERE remote = ?1 AND parent_path = ?2 AND path <> '/'",
                )?;
                let rows = statement.query_map(params![remote, parent], |row| row.get(0))?;
                rows.collect::<rusqlite::Result<_>>()?
            };
            let kept: std::collections::HashSet<&str> =
                children.iter().map(|child| child.path.as_str()).collect();
            let removed: Vec<String> = existing
                .into_iter()
                .filter(|path| !kept.contains(path.as_str()))
                .collect();
            for path in &removed {
                tx.execute(
                    &format!("DELETE FROM items WHERE {SUBTREE}"),
                    params![remote, path],
                )?;
            }
            for child in children {
                let previous: Option<(String, i64)> = tx
                    .query_row(
                        "SELECT id, listed_ms FROM items WHERE remote = ?1 AND path = ?2",
                        params![remote, child.path],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .optional()?;
                let mut row = child.clone();
                match previous {
                    // Same item at the same path: keep what Kura knows below it.
                    Some((id, listed)) if id == child.item.id => row.listed_ms = listed,
                    // A different item took this path: forget the old subtree.
                    Some(_) => {
                        tx.execute(
                            &format!("DELETE FROM items WHERE {SUBTREE} AND path <> ?2"),
                            params![remote, child.path],
                        )?;
                    }
                    None => {}
                }
                insert_item(&tx, remote, &row)?;
            }
            tx.execute(
                "UPDATE items SET listed_ms = ?3 WHERE remote = ?1 AND path = ?2",
                params![remote, parent, listed_ms],
            )?;
            tx.commit()?;
            Ok(removed)
        })
    }

    /// Removes an item and everything under it; returns the copies that were
    /// recorded there (also removed).
    pub(crate) fn delete_subtree(&self, remote: &str, path: &str) -> Result<Vec<CopyRow>> {
        self.with(|conn| {
            let tx = conn.transaction()?;
            let copies = {
                let mut statement = tx.prepare(&format!(
                    "SELECT {COPY_COLUMNS} FROM copies WHERE {SUBTREE}"
                ))?;
                let rows = statement.query_map(params![remote, path], read_copy)?;
                rows.collect::<rusqlite::Result<Vec<_>>>()?
            };
            for table in ["items", "copies", "conflicts"] {
                tx.execute(
                    &format!("DELETE FROM {table} WHERE {SUBTREE}"),
                    params![remote, path],
                )?;
            }
            tx.commit()?;
            Ok(copies)
        })
    }

    /// Moves an item and everything under it to a new path.
    pub(crate) fn move_subtree(&self, remote: &str, from: &str, to: &str) -> Result<()> {
        if from == names::ROOT || from == to {
            return Ok(());
        }
        let to_parent = names::parent(to);
        self.with(|conn| {
            let tx = conn.transaction()?;
            for table in ["items", "copies", "conflicts"] {
                tx.execute(&format!("DELETE FROM {table} WHERE {SUBTREE}"), params![remote, to])?;
            }
            tx.execute(
                &format!(
                    "UPDATE items SET
                       parent_path = CASE WHEN path = ?2 THEN ?4 ELSE ?3 || substr(parent_path, length(?2) + 1) END,
                       path = ?3 || substr(path, length(?2) + 1)
                     WHERE {SUBTREE}"
                ),
                params![remote, from, to, to_parent],
            )?;
            for table in ["copies", "conflicts"] {
                tx.execute(
                    &format!(
                        "UPDATE {table} SET path = ?3 || substr(path, length(?2) + 1) WHERE {SUBTREE}"
                    ),
                    params![remote, from, to],
                )?;
            }
            tx.commit()
        })
    }

    // Copies

    pub(crate) fn copy(&self, remote: &str, path: &str) -> Result<Option<CopyRow>> {
        self.with(|conn| {
            conn.query_row(
                &format!("SELECT {COPY_COLUMNS} FROM copies WHERE remote = ?1 AND path = ?2"),
                params![remote, path],
                read_copy,
            )
            .optional()
        })
    }

    pub(crate) fn put_copy(&self, remote: &str, row: &CopyRow) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO copies (remote, path, id, base_hash, base_revision,
                   local_size, local_mtime_ns, downloaded_ms, read_only, checked_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    remote,
                    row.path,
                    row.id,
                    row.base_hash,
                    row.base_revision,
                    row.local_size as i64,
                    row.local_mtime_ns,
                    row.downloaded_ms,
                    row.read_only,
                    row.checked_ms
                ],
            )
            .map(|_| ())
        })
    }

    // Conflicts

    pub(crate) fn conflict(&self, remote: &str, path: &str) -> Result<Option<ConflictRow>> {
        self.with(|conn| {
            conn.query_row(
                &format!(
                    "SELECT {CONFLICT_COLUMNS} FROM conflicts WHERE remote = ?1 AND path = ?2"
                ),
                params![remote, path],
                read_conflict,
            )
            .optional()
        })
    }

    pub(crate) fn conflicts(&self) -> Result<Vec<ConflictRow>> {
        self.with(|conn| {
            let mut statement = conn.prepare(&format!(
                "SELECT {CONFLICT_COLUMNS} FROM conflicts ORDER BY detected_ms DESC"
            ))?;
            let rows = statement.query_map([], read_conflict)?;
            rows.collect()
        })
    }

    pub(crate) fn put_conflict(&self, row: &ConflictRow) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO conflicts (remote, path, id, remote_revision, remote_hash,
                   remote_copy, detected_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    row.remote,
                    row.path,
                    row.id,
                    row.remote_revision,
                    row.remote_hash,
                    row.remote_copy,
                    row.detected_ms
                ],
            )
            .map(|_| ())
        })
    }

    pub(crate) fn delete_conflict(&self, remote: &str, path: &str) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "DELETE FROM conflicts WHERE remote = ?1 AND path = ?2",
                params![remote, path],
            )
            .map(|_| ())
        })
    }
}

const ITEM_COLUMNS: &str = "path, parent_path, id, name, is_dir, size, modified_ms, mime_type, revision, hash, provider_path, target_id, export_mime, listed_ms";
const COPY_COLUMNS: &str = "path, id, base_hash, base_revision, local_size, local_mtime_ns, downloaded_ms, read_only, checked_ms";
const CONFLICT_COLUMNS: &str =
    "remote, path, id, remote_revision, remote_hash, remote_copy, detected_ms";

fn insert_item(conn: &Connection, remote: &str, row: &ItemRow) -> rusqlite::Result<()> {
    let item = &row.item;
    conn.execute(
        "INSERT OR REPLACE INTO items (remote, path, parent_path, id, name, is_dir, size,
           modified_ms, mime_type, revision, hash, provider_path, target_id, export_mime, listed_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        params![
            remote,
            row.path,
            row.parent_path,
            item.id,
            item.name,
            item.is_dir,
            item.size as i64,
            item.modified_ms,
            item.mime_type,
            item.revision,
            item.hash,
            item.provider_path,
            item.target_id,
            item.export_mime,
            row.listed_ms
        ],
    )
    .map(|_| ())
}

fn read_remote(row: &Row<'_>) -> rusqlite::Result<Option<RemoteRow>> {
    let kind: String = row.get(1)?;
    let Some(kind) = ProviderKind::parse(&kind) else {
        return Ok(None);
    };
    Ok(Some(RemoteRow {
        name: row.get(0)?,
        kind,
        email: row.get(2)?,
        display_name: row.get(3)?,
        drive_type: row.get(4)?,
        created_ms: row.get(5)?,
    }))
}

fn read_item(row: &Row<'_>) -> rusqlite::Result<ItemRow> {
    Ok(ItemRow {
        path: row.get(0)?,
        parent_path: row.get(1)?,
        item: RemoteItem {
            id: row.get(2)?,
            name: row.get(3)?,
            is_dir: row.get(4)?,
            size: row.get::<_, i64>(5)?.max(0) as u64,
            modified_ms: row.get(6)?,
            mime_type: row.get(7)?,
            revision: row.get(8)?,
            hash: row.get(9)?,
            provider_path: row.get(10)?,
            target_id: row.get(11)?,
            export_mime: row.get(12)?,
        },
        listed_ms: row.get(13)?,
    })
}

fn read_copy(row: &Row<'_>) -> rusqlite::Result<CopyRow> {
    Ok(CopyRow {
        path: row.get(0)?,
        id: row.get(1)?,
        base_hash: row.get(2)?,
        base_revision: row.get(3)?,
        local_size: row.get::<_, i64>(4)?.max(0) as u64,
        local_mtime_ns: row.get(5)?,
        downloaded_ms: row.get(6)?,
        read_only: row.get(7)?,
        checked_ms: row.get(8)?,
    })
}

fn read_conflict(row: &Row<'_>) -> rusqlite::Result<ConflictRow> {
    Ok(ConflictRow {
        remote: row.get(0)?,
        path: row.get(1)?,
        id: row.get(2)?,
        remote_revision: row.get(3)?,
        remote_hash: row.get(4)?,
        remote_copy: row.get(5)?,
        detected_ms: row.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(path: &str, id: &str, is_dir: bool) -> ItemRow {
        ItemRow {
            path: path.into(),
            parent_path: names::parent(path),
            item: RemoteItem {
                id: id.into(),
                name: names::file_name(path).into(),
                is_dir,
                ..RemoteItem::default()
            },
            listed_ms: 0,
        }
    }

    fn copy(path: &str) -> CopyRow {
        CopyRow {
            path: path.into(),
            id: "x".into(),
            base_hash: "h".into(),
            base_revision: String::new(),
            local_size: 1,
            local_mtime_ns: 1,
            downloaded_ms: 1,
            read_only: false,
            checked_ms: 0,
        }
    }

    #[test]
    fn replacing_children_drops_vanished_subtrees_and_keeps_listed_ones() {
        let store = Store::in_memory().unwrap();
        store.put_item("r", &row("/", "root", true)).unwrap();
        store
            .replace_children("r", "/", &[row("/a", "1", true), row("/b", "2", true)], 10)
            .unwrap();
        store
            .replace_children("r", "/a", &[row("/a/x", "3", false)], 20)
            .unwrap();
        store
            .replace_children("r", "/b", &[row("/b/y", "4", false)], 20)
            .unwrap();

        let removed = store
            .replace_children("r", "/", &[row("/a", "1", true)], 30)
            .unwrap();
        assert_eq!(removed, vec!["/b".to_owned()]);
        assert!(store.item("r", "/b/y").unwrap().is_none());
        assert_eq!(store.item("r", "/a").unwrap().unwrap().listed_ms, 20);
        assert!(store.item("r", "/a/x").unwrap().is_some());
        assert_eq!(store.item("r", "/").unwrap().unwrap().listed_ms, 30);
    }

    #[test]
    fn a_different_item_at_a_path_forgets_the_old_subtree() {
        let store = Store::in_memory().unwrap();
        store.put_item("r", &row("/", "root", true)).unwrap();
        store
            .replace_children("r", "/", &[row("/a", "1", true)], 10)
            .unwrap();
        store
            .replace_children("r", "/a", &[row("/a/x", "3", false)], 10)
            .unwrap();
        store
            .replace_children("r", "/", &[row("/a", "9", true)], 20)
            .unwrap();
        assert!(store.item("r", "/a/x").unwrap().is_none());
        assert_eq!(store.item("r", "/a").unwrap().unwrap().listed_ms, 0);
    }

    #[test]
    fn moving_a_subtree_rewrites_paths_without_touching_siblings() {
        let store = Store::in_memory().unwrap();
        for (path, id, dir) in [("/a", "1", true), ("/a/x", "2", false), ("/ab", "3", false)] {
            store.put_item("r", &row(path, id, dir)).unwrap();
        }
        store.put_copy("r", &copy("/a/x")).unwrap();
        store.move_subtree("r", "/a", "/z/a").unwrap();
        let moved = store.item("r", "/z/a/x").unwrap().unwrap();
        assert_eq!(moved.parent_path, "/z/a");
        assert_eq!(store.item("r", "/z/a").unwrap().unwrap().parent_path, "/z");
        assert!(store.item("r", "/ab").unwrap().is_some());
        assert!(store.copy("r", "/z/a/x").unwrap().is_some());
        assert!(store.copy("r", "/a/x").unwrap().is_none());
    }

    #[test]
    fn deleting_a_subtree_returns_its_copies() {
        let store = Store::in_memory().unwrap();
        store.put_item("r", &row("/a", "1", true)).unwrap();
        store.put_item("r", &row("/a/x", "2", false)).unwrap();
        store.put_copy("r", &copy("/a/x")).unwrap();
        store.put_copy("r", &copy("/ab")).unwrap();
        let removed = store.delete_subtree("r", "/a").unwrap();
        assert_eq!(removed.len(), 1);
        assert!(store.copy("r", "/ab").unwrap().is_some());
    }

    #[test]
    fn remotes_round_trip() {
        let store = Store::in_memory().unwrap();
        let remote = RemoteRow {
            name: "Work".into(),
            kind: ProviderKind::OneDrive,
            email: "a@b.c".into(),
            display_name: "A".into(),
            drive_type: "business".into(),
            created_ms: 5,
        };
        store.put_remote(&remote).unwrap();
        assert_eq!(store.remote("work").unwrap().unwrap(), remote);
        store.delete_remote("Work").unwrap();
        assert!(store.remotes().unwrap().is_empty());
    }
}
