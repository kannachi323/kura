//! Free and total space on the volume that holds a folder, for the Files
//! status bar.

use serde::Serialize;

use crate::error::{ApiError, ApiResult};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VolumeSpace {
    pub available_bytes: u64,
    pub total_bytes: u64,
}

// statvfs field widths differ between platforms, so some casts are no-ops.
#[cfg(unix)]
#[allow(clippy::unnecessary_cast)]
pub fn volume_space(path: &str) -> ApiResult<VolumeSpace> {
    use std::ffi::CString;

    let c_path =
        CString::new(path).map_err(|_| ApiError::Message(format!("Invalid path: {path}")))?;
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: `c_path` is a valid NUL-terminated string and `stats` is
    // written by statvfs before it is read.
    let result = unsafe { libc::statvfs(c_path.as_ptr(), stats.as_mut_ptr()) };
    if result != 0 {
        return Err(ApiError::Message(format!(
            "Could not read free space for {path}: {}",
            std::io::Error::last_os_error()
        )));
    }
    // SAFETY: statvfs succeeded, so `stats` is initialised.
    let stats = unsafe { stats.assume_init() };
    let fragment = stats.f_frsize as u64;
    Ok(VolumeSpace {
        // Space an unprivileged user can use, as Finder reports it.
        available_bytes: (stats.f_bavail as u64).saturating_mul(fragment),
        total_bytes: (stats.f_blocks as u64).saturating_mul(fragment),
    })
}

#[cfg(not(unix))]
pub fn volume_space(path: &str) -> ApiResult<VolumeSpace> {
    Err(ApiError::Message(format!(
        "Free space is not available for {path} on this platform."
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_space_for_an_existing_folder() {
        let dir = tempfile::tempdir().expect("temp dir");
        let space = volume_space(&dir.path().display().to_string()).expect("space");
        assert!(space.total_bytes > 0);
        assert!(space.available_bytes <= space.total_bytes);
    }
}
