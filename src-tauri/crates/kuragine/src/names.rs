//! Remote paths and the local names cloud items get.
//!
//! A remote path ("rel") is `/`-rooted and uses `/` separators whatever the
//! platform: `/`, `/Reports`, `/Reports/q3.xlsx`. It is the item's path under
//! its remote's local folder, after names were made safe and unique, so it
//! maps one-to-one to a local path.
use std::{
    collections::HashSet,
    path::{Component, Path, PathBuf},
};

use crate::{Error, Result};

pub const ROOT: &str = "/";

pub fn join(parent: &str, name: &str) -> String {
    if parent == ROOT || parent.is_empty() {
        format!("/{name}")
    } else {
        format!("{parent}/{name}")
    }
}

pub fn parent(rel: &str) -> String {
    match rel.rfind('/') {
        Some(0) | None => ROOT.to_owned(),
        Some(index) => rel[..index].to_owned(),
    }
}

pub fn file_name(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or_default()
}

/// Whether `rel` is `ancestor` or inside it.
pub fn is_within(rel: &str, ancestor: &str) -> bool {
    ancestor == ROOT
        || rel == ancestor
        || (rel.starts_with(ancestor) && rel.as_bytes().get(ancestor.len()) == Some(&b'/'))
}

/// Rewrites `rel` from under `from` to under `to`.
pub fn rebase(rel: &str, from: &str, to: &str) -> String {
    if rel == from {
        return to.to_owned();
    }
    let rest = &rel[from.len()..];
    if to == ROOT {
        rest.to_owned()
    } else {
        format!("{to}{rest}")
    }
}

/// Converts a path relative to a remote's folder into a remote path.
pub fn rel_from_components(relative: &Path) -> Result<String> {
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => parts.push(
                part.to_str()
                    .ok_or_else(|| Error::invalid("File names must be valid Unicode."))?
                    .to_owned(),
            ),
            Component::CurDir => {}
            _ => return Err(Error::invalid("Cloud paths cannot leave their folder.")),
        }
    }
    Ok(if parts.is_empty() {
        ROOT.to_owned()
    } else {
        format!("/{}", parts.join("/"))
    })
}

pub fn local_path(remote_root: &Path, rel: &str) -> PathBuf {
    let mut path = remote_root.to_path_buf();
    for part in rel.split('/').filter(|part| !part.is_empty()) {
        path.push(part);
    }
    path
}

/// Makes a cloud name usable as a local file name. Drive allows `/` in names,
/// and Windows rejects several more characters, so those become look-alikes.
pub fn safe_local_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        let mapped = match ch {
            '/' => '／',
            '\\' => '＼',
            '\0'..='\u{1f}' => '_',
            ':' if cfg!(windows) => '：',
            '*' if cfg!(windows) => '＊',
            '?' if cfg!(windows) => '？',
            '"' if cfg!(windows) => '＂',
            '<' if cfg!(windows) => '＜',
            '>' if cfg!(windows) => '＞',
            '|' if cfg!(windows) => '｜',
            other => other,
        };
        out.push(mapped);
    }
    if out.is_empty() || out == "." || out == ".." {
        out.insert(0, '_');
    }
    if cfg!(windows) {
        if out.ends_with('.') || out.ends_with(' ') {
            out.push('_');
        }
        let stem = out
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || ((stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem.len() == 4
                && stem.as_bytes()[3].is_ascii_digit());
        if reserved {
            out.insert(0, '_');
        }
    }
    out
}

/// Gives every entry in one folder a distinct local name. Names that collide
/// (exact duplicates, which Drive allows, or names differing only in case,
/// which most local disks treat as the same) get ` (2)`, ` (3)`, … in the order
/// of `keys`, so the same listing always produces the same names.
pub fn unique_names(names: &[String], keys: &[String]) -> Vec<String> {
    let mut order: Vec<usize> = (0..names.len()).collect();
    order.sort_by(|&left, &right| {
        names[left]
            .to_lowercase()
            .cmp(&names[right].to_lowercase())
            .then_with(|| keys[left].cmp(&keys[right]))
    });
    let mut taken: HashSet<String> = HashSet::new();
    let mut result = vec![String::new(); names.len()];
    // Unique names keep their spelling even if a later suffix would collide.
    let mut counts = std::collections::HashMap::<String, usize>::new();
    for name in names {
        *counts.entry(name.to_lowercase()).or_default() += 1;
    }
    for &index in &order {
        if counts[&names[index].to_lowercase()] == 1 {
            taken.insert(names[index].to_lowercase());
            result[index] = names[index].clone();
        }
    }
    for &index in &order {
        if !result[index].is_empty() {
            continue;
        }
        let name = &names[index];
        let mut candidate = name.clone();
        let mut n = 1;
        while taken.contains(&candidate.to_lowercase()) {
            n += 1;
            candidate = numbered(name, n);
        }
        taken.insert(candidate.to_lowercase());
        result[index] = candidate;
    }
    result
}

pub fn numbered(name: &str, n: usize) -> String {
    let (stem, extension) = split_extension(name);
    match extension {
        Some(extension) => format!("{stem} ({n}).{extension}"),
        None => format!("{stem} ({n})"),
    }
}

pub fn split_extension(name: &str) -> (&str, Option<&str>) {
    match name.rfind('.') {
        Some(index) if index > 0 && index + 1 < name.len() => {
            (&name[..index], Some(&name[index + 1..]))
        }
        _ => (name, None),
    }
}

/// Files editors and the OS create next to documents. They are never uploaded.
pub fn is_scratch_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    name.starts_with("~$")
        || name.starts_with(".~lock.")
        || name.starts_with(".#")
        || name.ends_with('~')
        || matches!(
            lower.as_str(),
            ".ds_store" | "thumbs.db" | "desktop.ini" | ".localized"
        )
        || lower.starts_with("._")
        || [
            ".tmp",
            ".temp",
            ".swp",
            ".swo",
            ".swx",
            ".crdownload",
            ".part",
            ".partial",
            ".download",
        ]
        .iter()
        .any(|suffix| lower.ends_with(suffix))
        || lower.contains(".sb-") // macOS safe-save scratch files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_and_splits_remote_paths() {
        assert_eq!(join(ROOT, "a"), "/a");
        assert_eq!(join("/a", "b.txt"), "/a/b.txt");
        assert_eq!(parent("/a/b.txt"), "/a");
        assert_eq!(parent("/a"), ROOT);
        assert_eq!(file_name("/a/b.txt"), "b.txt");
        assert!(is_within("/a/b", "/a"));
        assert!(!is_within("/ab", "/a"));
        assert!(is_within("/anything", ROOT));
        assert_eq!(rebase("/a/b/c", "/a/b", "/x"), "/x/c");
        assert_eq!(rebase("/a/b", "/a/b", "/x"), "/x");
        assert_eq!(rebase("/a/b", "/a", ROOT), "/b");
    }

    #[test]
    fn rejects_paths_that_leave_the_remote() {
        assert!(rel_from_components(Path::new("a/../b")).is_err());
        assert_eq!(rel_from_components(Path::new("a/b")).unwrap(), "/a/b");
        assert_eq!(rel_from_components(Path::new("")).unwrap(), ROOT);
    }

    #[test]
    fn makes_cloud_names_safe() {
        assert_eq!(safe_local_name("a/b"), "a／b");
        assert_eq!(safe_local_name(".."), "_..");
        assert_eq!(safe_local_name(""), "_");
        assert_eq!(safe_local_name("ok.txt"), "ok.txt");
    }

    #[test]
    fn numbers_colliding_names_stably() {
        let names = vec![
            "Report.pdf".to_owned(),
            "report.pdf".to_owned(),
            "Notes".to_owned(),
            "Report.pdf".to_owned(),
        ];
        let keys = vec![
            "b".to_owned(),
            "a".to_owned(),
            "z".to_owned(),
            "c".to_owned(),
        ];
        let unique = unique_names(&names, &keys);
        assert_eq!(unique[1], "report.pdf");
        assert_eq!(unique[0], "Report (2).pdf");
        assert_eq!(unique[3], "Report (3).pdf");
        assert_eq!(unique[2], "Notes");
    }

    #[test]
    fn suffixes_skip_names_already_used() {
        let names = vec![
            "a.txt".to_owned(),
            "a.txt".to_owned(),
            "a (2).txt".to_owned(),
        ];
        let keys = vec!["1".to_owned(), "2".to_owned(), "3".to_owned()];
        let unique = unique_names(&names, &keys);
        assert_eq!(unique, vec!["a.txt", "a (3).txt", "a (2).txt"]);
    }

    #[test]
    fn recognizes_scratch_files() {
        for name in [
            "~$report.docx",
            ".DS_Store",
            "a.txt.swp",
            "notes.tmp",
            "x~",
            "._a",
        ] {
            assert!(is_scratch_name(name), "{name}");
        }
        for name in ["report.docx", ".gitignore", "temp.txt"] {
            assert!(!is_scratch_name(name), "{name}");
        }
    }
}
