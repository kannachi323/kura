//! Reads Finder's Put Back records from a Trash folder's `.DS_Store`.
//!
//! When Finder trashes an item it records, in the Trash's `.DS_Store`, where
//! the item came from: `ptbL` (the folder, relative to the volume root) and
//! `ptbN` (the original name). Kura reads them to put back items it didn't
//! trash itself. The format is a buddy-allocated file holding a B-tree of
//! records; see "DS_Store format" (Mark Mentovai, Wim Lewis). Anything that
//! doesn't parse is ignored.

use std::collections::HashMap;

/// Where a trashed item came from, keyed by its name in the Trash.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PutBackRecord {
    /// Folder relative to the volume root, e.g. "Users/me/Documents/".
    pub location: Option<String>,
    /// The item's original name.
    pub name: Option<String>,
}

pub fn put_back_records(data: &[u8]) -> HashMap<String, PutBackRecord> {
    let mut records = HashMap::new();
    let mut seen = 0;
    let _ = read_records(data, &mut records, &mut seen);
    records
}

/// How many records of any kind the file holds, or None if it doesn't parse.
#[cfg(test)]
fn record_count(data: &[u8]) -> Option<usize> {
    let mut seen = 0;
    read_records(data, &mut HashMap::new(), &mut seen).map(|()| seen)
}

fn read_records(
    data: &[u8],
    out: &mut HashMap<String, PutBackRecord>,
    seen: &mut usize,
) -> Option<()> {
    // Offsets in the file are relative to byte 4.
    if data.get(4..8)? != b"Bud1" {
        return None;
    }
    let file = Reader { data, base: 4 };
    // The header's root-block offset sits at absolute byte 8, which is 4 in
    // the file's own (base-4) offsets.
    let root_offset = file.u32(4)? as usize;
    let mut cursor = root_offset;
    let block_count = file.u32(cursor)? as usize;
    cursor += 8;
    let mut addresses = Vec::with_capacity(block_count);
    for index in 0..block_count {
        addresses.push(file.u32(cursor + index * 4)?);
    }
    // The address table is padded to a multiple of 256 entries.
    cursor += block_count.div_ceil(256).max(1) * 256 * 4;
    let directory_count = file.u32(cursor)? as usize;
    cursor += 4;
    let mut master = None;
    for _ in 0..directory_count {
        let length = *file.bytes(cursor, 1)?.first()? as usize;
        let name = file.bytes(cursor + 1, length)?;
        let block = file.u32(cursor + 1 + length)?;
        cursor += 1 + length + 4;
        if name == b"DSDB" {
            master = Some(block as usize);
        }
    }
    let block_offset = |id: usize| -> Option<usize> {
        addresses.get(id).map(|address| (address & !0x1f) as usize)
    };
    let master = block_offset(master?)?;
    let root_node = file.u32(master)? as usize;
    read_node(&file, &block_offset, root_node, out, seen, 0)
}

fn read_node(
    file: &Reader,
    block_offset: &dyn Fn(usize) -> Option<usize>,
    node: usize,
    out: &mut HashMap<String, PutBackRecord>,
    seen: &mut usize,
    depth: usize,
) -> Option<()> {
    if depth > 32 {
        return None;
    }
    let offset = block_offset(node)?;
    let right_child = file.u32(offset)? as usize;
    let count = file.u32(offset + 4)? as usize;
    let mut cursor = offset + 8;
    for _ in 0..count {
        if right_child != 0 {
            let child = file.u32(cursor)? as usize;
            cursor += 4;
            read_node(file, block_offset, child, out, seen, depth + 1)?;
        }
        cursor = read_record(file, cursor, out)?;
        *seen += 1;
    }
    if right_child != 0 {
        read_node(file, block_offset, right_child, out, seen, depth + 1)?;
    }
    Some(())
}

/// Reads one record at `cursor` and returns where the next one starts.
fn read_record(
    file: &Reader,
    mut cursor: usize,
    out: &mut HashMap<String, PutBackRecord>,
) -> Option<usize> {
    let name_length = file.u32(cursor)? as usize;
    let name = file.utf16(cursor + 4, name_length)?;
    cursor += 4 + name_length * 2;
    let code = file.bytes(cursor, 4)?;
    let kind = file.bytes(cursor + 4, 4)?;
    cursor += 8;
    let value = match kind {
        b"ustr" => {
            let length = file.u32(cursor)? as usize;
            let text = file.utf16(cursor + 4, length)?;
            cursor += 4 + length * 2;
            Some(text)
        }
        b"blob" => {
            cursor += 4 + file.u32(cursor)? as usize;
            None
        }
        b"long" | b"shor" | b"type" => {
            cursor += 4;
            None
        }
        b"bool" => {
            cursor += 1;
            None
        }
        b"comp" | b"dutc" => {
            cursor += 8;
            None
        }
        _ => return None,
    };
    if let Some(value) = value {
        match code {
            b"ptbL" => out.entry(name).or_default().location = Some(value),
            b"ptbN" => out.entry(name).or_default().name = Some(value),
            _ => {}
        }
    }
    Some(cursor)
}

struct Reader<'a> {
    data: &'a [u8],
    base: usize,
}

impl Reader<'_> {
    fn bytes(&self, offset: usize, length: usize) -> Option<&[u8]> {
        let start = self.base.checked_add(offset)?;
        self.data.get(start..start.checked_add(length)?)
    }

    fn u32(&self, offset: usize) -> Option<u32> {
        let bytes = self.bytes(offset, 4)?;
        Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn utf16(&self, offset: usize, units: usize) -> Option<String> {
        let bytes = self.bytes(offset, units.checked_mul(2)?)?;
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect();
        Some(String::from_utf16_lossy(&units))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn utf16(text: &str) -> Vec<u8> {
        text.encode_utf16()
            .flat_map(|unit| unit.to_be_bytes())
            .collect()
    }

    fn record(file: &str, code: &[u8; 4], value: &str) -> Vec<u8> {
        let mut out = (file.encode_utf16().count() as u32).to_be_bytes().to_vec();
        out.extend(utf16(file));
        out.extend(code);
        out.extend(b"ustr");
        out.extend((value.encode_utf16().count() as u32).to_be_bytes());
        out.extend(utf16(value));
        out
    }

    /// A minimal `.DS_Store` with one leaf node holding `records`.
    pub(crate) fn ds_store(records: &[Vec<u8>]) -> Vec<u8> {
        const ROOT: usize = 0x40;
        const MASTER: usize = 0x1400;
        const LEAF: usize = 0x1800;
        let mut file = vec![0u8; 4 + LEAF + 0x800];
        let put = |file: &mut Vec<u8>, at: usize, bytes: &[u8]| {
            file[4 + at..4 + at + bytes.len()].copy_from_slice(bytes);
        };
        file[..4].copy_from_slice(&[0, 0, 0, 1]);
        file[4..8].copy_from_slice(b"Bud1");
        file[8..12].copy_from_slice(&(ROOT as u32).to_be_bytes());
        // Root block: three blocks, then the DSDB directory entry.
        let mut root = Vec::new();
        root.extend(3u32.to_be_bytes());
        root.extend(0u32.to_be_bytes());
        for offset in [ROOT, MASTER, LEAF] {
            root.extend(((offset as u32) | 0x0b).to_be_bytes());
        }
        root.resize(8 + 256 * 4, 0);
        root.extend(1u32.to_be_bytes());
        root.push(4);
        root.extend(b"DSDB");
        root.extend(1u32.to_be_bytes());
        put(&mut file, ROOT, &root);
        // Master block: the B-tree's root node is block 2.
        let mut master = Vec::new();
        for value in [2u32, 0, records.len() as u32, 1, 0x1000] {
            master.extend(value.to_be_bytes());
        }
        put(&mut file, MASTER, &master);
        let mut leaf = 0u32.to_be_bytes().to_vec();
        leaf.extend((records.len() as u32).to_be_bytes());
        for record in records {
            leaf.extend(record);
        }
        put(&mut file, LEAF, &leaf);
        file
    }

    pub(crate) fn put_back_store(trash_name: &str, location: &str, name: &str) -> Vec<u8> {
        ds_store(&[
            record(trash_name, b"ptbL", location),
            record(trash_name, b"ptbN", name),
        ])
    }

    #[test]
    fn reads_finder_put_back_records() {
        let records = put_back_records(&put_back_store(
            "report 2.pdf",
            "Users/me/Documents/",
            "report.pdf",
        ));
        assert_eq!(
            records.get("report 2.pdf"),
            Some(&PutBackRecord {
                location: Some("Users/me/Documents/".into()),
                name: Some("report.pdf".into()),
            })
        );
    }

    /// Opt-in check against real files: KURA_DS_STORES=path1:path2 cargo test
    #[test]
    fn parses_real_ds_stores_when_given() {
        let Ok(paths) = std::env::var("KURA_DS_STORES") else {
            return;
        };
        for path in paths.split(':') {
            let data = std::fs::read(path).expect("read .DS_Store");
            let count = record_count(&data);
            println!("{path}: {count:?} records");
            for (name, record) in put_back_records(&data) {
                println!("  put back {name}: {record:?}");
            }
            assert!(count.is_some_and(|count| count > 0), "{path} didn't parse");
        }
    }

    #[test]
    fn ignores_files_that_are_not_ds_stores() {
        assert!(put_back_records(b"not a ds_store").is_empty());
        let mut truncated = put_back_store("a", "b/", "c");
        truncated.truncate(0x900);
        assert!(put_back_records(&truncated).is_empty());
    }
}
