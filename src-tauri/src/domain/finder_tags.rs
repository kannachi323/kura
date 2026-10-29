//! Finder tags (colour labels).
//!
//! macOS keeps a file's tags in the `com.apple.metadata:_kMDItemUserTags`
//! extended attribute: a binary property list holding an array of strings
//! like "Red\n6" (name, newline, colour index). Kura reads that attribute
//! directly, only for rows on screen, and writes tags through `NSURL` so
//! Spotlight and Finder see the change.

use serde::Serialize;

use crate::error::{ApiError, ApiResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinderTag {
    pub name: String,
    /// Finder's colour index: 0 none, 1 grey, 2 green, 3 purple, 4 blue,
    /// 5 yellow, 6 red, 7 orange.
    pub color: u8,
}

/// The standard colour tags, in Finder's menu order.
pub const COLOR_TAGS: [(&str, u8); 7] = [
    ("Red", 6),
    ("Orange", 7),
    ("Yellow", 5),
    ("Green", 2),
    ("Blue", 4),
    ("Purple", 3),
    ("Gray", 1),
];

#[cfg(target_os = "macos")]
const TAGS_ATTRIBUTE: &[u8] = b"com.apple.metadata:_kMDItemUserTags\0";

/// The tags on `path`. Empty when it has none or they can't be read.
#[cfg(target_os = "macos")]
pub fn read_tags(path: &std::path::Path) -> Vec<FinderTag> {
    use std::os::unix::ffi::OsStrExt;

    let Ok(c_path) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
        return Vec::new();
    };
    let name = TAGS_ATTRIBUTE.as_ptr().cast();
    // SAFETY: both strings are NUL-terminated; a null buffer asks for the size.
    let size = unsafe {
        libc::getxattr(
            c_path.as_ptr(),
            name,
            std::ptr::null_mut(),
            0,
            0,
            libc::XATTR_NOFOLLOW,
        )
    };
    if size <= 0 {
        return Vec::new();
    }
    let mut buffer = vec![0u8; size as usize];
    // SAFETY: `buffer` has room for `size` bytes.
    let read = unsafe {
        libc::getxattr(
            c_path.as_ptr(),
            name,
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            0,
            libc::XATTR_NOFOLLOW,
        )
    };
    if read <= 0 {
        return Vec::new();
    }
    buffer.truncate(read as usize);
    parse_tag_strings(&buffer)
        .unwrap_or_default()
        .into_iter()
        .map(|text| parse_tag(&text))
        .collect()
}

#[cfg(not(target_os = "macos"))]
pub fn read_tags(_path: &std::path::Path) -> Vec<FinderTag> {
    Vec::new()
}

/// Replaces the tags on `path` with `names`.
#[cfg(target_os = "macos")]
pub fn write_tags(path: &std::path::Path, names: &[String]) -> ApiResult<()> {
    use objc2::rc::autoreleasepool;
    use objc2_foundation::{NSArray, NSString, NSURLTagNamesKey, NSURL};

    autoreleasepool(|_| {
        let url = NSURL::fileURLWithPath(&NSString::from_str(&path.display().to_string()));
        let values: Vec<_> = names.iter().map(|name| NSString::from_str(name)).collect();
        let array = NSArray::from_retained_slice(&values);
        // SAFETY: NSURLTagNamesKey takes an NSArray of NSString.
        unsafe { url.setResourceValue_forKey_error(Some(&array), NSURLTagNamesKey) }.map_err(
            |error| {
                ApiError::Message(format!(
                    "Could not tag {}: {}",
                    path.display(),
                    error.localizedDescription()
                ))
            },
        )
    })
}

#[cfg(not(target_os = "macos"))]
pub fn write_tags(_path: &std::path::Path, _names: &[String]) -> ApiResult<()> {
    Err(ApiError::Message(
        "Tags are only available on macOS.".to_owned(),
    ))
}

/// Adds or removes the tag `name` on each of `paths`.
pub fn set_tag(paths: &[String], name: &str, enabled: bool) -> ApiResult<()> {
    for path in paths {
        let path = std::path::Path::new(path);
        let mut names: Vec<String> = read_tags(path).into_iter().map(|tag| tag.name).collect();
        let present = names.iter().any(|existing| existing == name);
        if enabled == present {
            continue;
        }
        if enabled {
            names.push(name.to_owned());
        } else {
            names.retain(|existing| existing != name);
        }
        write_tags(path, &names)?;
    }
    Ok(())
}

fn parse_tag(text: &str) -> FinderTag {
    match text.rsplit_once('\n') {
        Some((name, color)) => FinderTag {
            name: name.to_owned(),
            color: color.trim().parse().unwrap_or(0),
        },
        None => FinderTag {
            color: COLOR_TAGS
                .iter()
                .find(|(standard, _)| *standard == text)
                .map(|(_, color)| *color)
                .unwrap_or(0),
            name: text.to_owned(),
        },
    }
}

/// Reads a binary property list whose top object is an array of strings.
fn parse_tag_strings(data: &[u8]) -> Option<Vec<String>> {
    if data.len() < 40 || &data[..8] != b"bplist00" {
        return None;
    }
    let trailer = &data[data.len() - 32..];
    let offset_size = trailer[6] as usize;
    let ref_size = trailer[7] as usize;
    let object_count = read_uint(&trailer[8..16])?;
    let top_object = read_uint(&trailer[16..24])?;
    let table_offset = read_uint(&trailer[24..32])?;
    let object_offset = |index: usize| -> Option<usize> {
        if index >= object_count {
            return None;
        }
        let start = table_offset.checked_add(index.checked_mul(offset_size)?)?;
        read_uint(data.get(start..start + offset_size)?)
    };
    let (marker, count, body) = object_header(data, object_offset(top_object)?)?;
    if marker != 0xA {
        return None;
    }
    (0..count)
        .map(|position| {
            let start = body + position * ref_size;
            let index = read_uint(data.get(start..start + ref_size)?)?;
            let (marker, length, body) = object_header(data, object_offset(index)?)?;
            match marker {
                0x5 => Some(String::from_utf8_lossy(data.get(body..body + length)?).into_owned()),
                0x6 => {
                    let units: Vec<u16> = data
                        .get(body..body + length * 2)?
                        .chunks_exact(2)
                        .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
                        .collect();
                    Some(String::from_utf16_lossy(&units))
                }
                _ => None,
            }
        })
        .collect()
}

/// An object's type nibble, its length and where its contents start.
fn object_header(data: &[u8], offset: usize) -> Option<(u8, usize, usize)> {
    let byte = *data.get(offset)?;
    let (marker, low) = (byte >> 4, (byte & 0x0F) as usize);
    if low != 0x0F {
        return Some((marker, low, offset + 1));
    }
    // A length of 15 or more follows as an integer object.
    let int_marker = *data.get(offset + 1)?;
    if int_marker >> 4 != 0x1 {
        return None;
    }
    let width = 1usize << (int_marker & 0x0F);
    let length = read_uint(data.get(offset + 2..offset + 2 + width)?)?;
    Some((marker, length, offset + 2 + width))
}

fn read_uint(bytes: &[u8]) -> Option<usize> {
    if bytes.is_empty() || bytes.len() > 8 {
        return None;
    }
    Some(
        bytes
            .iter()
            .fold(0u64, |value, byte| (value << 8) | *byte as u64) as usize,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `["Red\n6", "Projects"]`, as written by Finder.
    fn sample_plist() -> Vec<u8> {
        let mut data = b"bplist00".to_vec();
        let array_offset = data.len();
        data.extend([0xA2, 1, 2]);
        let red_offset = data.len();
        data.push(0x55);
        data.extend(b"Red\n6");
        let projects_offset = data.len();
        data.push(0x58);
        data.extend(b"Projects");
        let table_offset = data.len();
        data.extend([array_offset as u8, red_offset as u8, projects_offset as u8]);
        let mut trailer = vec![0u8; 6];
        trailer.extend([1, 1]);
        trailer.extend(3u64.to_be_bytes());
        trailer.extend(0u64.to_be_bytes());
        trailer.extend((table_offset as u64).to_be_bytes());
        data.extend(trailer);
        data
    }

    #[test]
    fn parses_finder_tag_plists() {
        let tags: Vec<FinderTag> = parse_tag_strings(&sample_plist())
            .expect("parse")
            .iter()
            .map(|text| parse_tag(text))
            .collect();
        assert_eq!(
            tags,
            vec![
                FinderTag {
                    name: "Red".into(),
                    color: 6
                },
                FinderTag {
                    name: "Projects".into(),
                    color: 0
                },
            ]
        );
    }

    #[test]
    fn rejects_malformed_data() {
        assert_eq!(parse_tag_strings(b"not a plist"), None);
        let mut truncated = sample_plist();
        truncated.truncate(30);
        assert_eq!(parse_tag_strings(&truncated), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn writes_and_reads_tags_on_a_real_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join("tagged.txt");
        std::fs::write(&file, b"x").expect("write");
        let paths = vec![file.display().to_string()];
        set_tag(&paths, "Red", true).expect("tag");
        assert_eq!(
            read_tags(&file),
            vec![FinderTag {
                name: "Red".into(),
                color: 6
            }]
        );
        set_tag(&paths, "Red", false).expect("untag");
        assert!(read_tags(&file).is_empty());
    }
}
