//! Sharing files through macOS services (AirDrop, Mail, Messages), as
//! Finder's Share menu does.

use crate::error::{ApiError, ApiResult};

/// Shares `paths` with `service` ("airdrop", "mail" or "messages"). AppKit
/// requires this to run on the main thread.
#[cfg(target_os = "macos")]
pub fn share_on_main_thread(paths: &[String], service: &str) -> ApiResult<()> {
    use objc2::rc::{autoreleasepool, Retained};
    use objc2_app_kit::{
        NSSharingService, NSSharingServiceNameComposeEmail, NSSharingServiceNameComposeMessage,
        NSSharingServiceNameSendViaAirDrop,
    };
    use objc2_foundation::{NSArray, NSString, NSURL};

    autoreleasepool(|_| {
        // SAFETY: these are AppKit's constant service names.
        let name = unsafe {
            match service {
                "airdrop" => NSSharingServiceNameSendViaAirDrop,
                "mail" => NSSharingServiceNameComposeEmail,
                "messages" => NSSharingServiceNameComposeMessage,
                other => return Err(ApiError::Message(format!("Unknown share service: {other}"))),
            }
        };
        let service = NSSharingService::sharingServiceNamed(name)
            .ok_or_else(|| ApiError::Message("This sharing service isn't available.".into()))?;
        let urls: Vec<_> = paths
            .iter()
            .map(|path| NSURL::fileURLWithPath(&NSString::from_str(path)))
            .collect();
        let items = NSArray::from_retained_slice(&urls);
        // SAFETY: an array of NSURL is an array of objects, and file URLs
        // conform to NSPasteboardWriting, as these methods require.
        unsafe {
            let items: Retained<NSArray> = Retained::cast_unchecked(items);
            if !service.canPerformWithItems(Some(&items)) {
                return Err(ApiError::Message(
                    "These items can't be shared that way.".into(),
                ));
            }
            service.performWithItems(&items);
        }
        Ok(())
    })
}

#[cfg(not(target_os = "macos"))]
pub fn share_on_main_thread(_paths: &[String], _service: &str) -> ApiResult<()> {
    Err(ApiError::Message(
        "Sharing is only available on macOS.".into(),
    ))
}
