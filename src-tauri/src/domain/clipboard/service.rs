use std::sync::{Arc, Mutex};

use super::{ClipboardOrigin, ClipboardPayload, ClipboardPayloadKind};
use crate::domain::file_transfer::now_epoch_ms;

pub trait NativeClipboard: Send + Sync {
    fn supported(&self) -> bool;
    fn start(&self, on_changed: Arc<dyn Fn() + Send + Sync + 'static>) -> bool;
    fn stop(&self);
    fn read_payload(&self) -> Option<ClipboardPayload>;
    fn write_payload(&self, payload: &ClipboardPayload) -> bool;
}

#[derive(Default)]
struct ClipboardState {
    local: ClipboardPayload,
    last_seen_fingerprint: Vec<u8>,
    next_revision: u64,
}

pub struct ClipboardService {
    native: Option<Arc<dyn NativeClipboard>>,
    state: Mutex<ClipboardState>,
}

impl ClipboardService {
    pub fn new(native: Option<Arc<dyn NativeClipboard>>) -> Arc<Self> {
        Arc::new(Self {
            native,
            state: Mutex::new(ClipboardState {
                next_revision: 1,
                ..ClipboardState::default()
            }),
        })
    }

    pub fn start(self: &Arc<Self>) -> bool {
        let Some(native) = &self.native else {
            return false;
        };
        if !native.supported() {
            return false;
        }
        let service = Arc::downgrade(self);
        native.start(Arc::new(move || {
            if let Some(service) = service.upgrade() {
                service.on_native_clipboard_changed();
            }
        }))
    }

    pub fn stop(&self) {
        if let Some(native) = &self.native {
            native.stop();
        }
    }

    pub fn current_local(&self) -> ClipboardPayload {
        self.state
            .lock()
            .expect("clipboard state lock")
            .local
            .clone()
    }

    pub fn set_local_kura_payload(&self, payload: ClipboardPayload) -> ClipboardPayload {
        let payload = self.finalize_payload(payload, ClipboardOrigin::LocalKura);
        self.set_local_payload(payload.clone());
        payload
    }

    pub fn finalize_payload(
        &self,
        mut payload: ClipboardPayload,
        origin: ClipboardOrigin,
    ) -> ClipboardPayload {
        payload.origin = origin;
        payload.created_unix_ms = now_epoch_ms();
        let mut state = self.state.lock().expect("clipboard state lock");
        payload.revision = state.next_revision;
        state.next_revision = state.next_revision.saturating_add(1);
        payload.payload_id = format!("{}:{}", payload.revision, payload.created_unix_ms);
        payload
    }

    pub fn fingerprint_for(payload: &ClipboardPayload) -> Vec<u8> {
        let kind = match payload.kind {
            ClipboardPayloadKind::Empty => 0,
            ClipboardPayloadKind::Text => 1,
            ClipboardPayloadKind::Html => 2,
            ClipboardPayloadKind::Image => 3,
            ClipboardPayloadKind::FileRefs => 4,
        };
        let mut out = format!("{kind}\n{}\n{}\n", payload.text, payload.html).into_bytes();
        for file_ref in &payload.file_refs {
            out.extend_from_slice(
                format!(
                    "{}\t{}\t{}\t{}\t{}\t{}\n",
                    file_ref.display_name,
                    file_ref.local_path,
                    file_ref.provider_type,
                    file_ref.remote_name,
                    file_ref.remote_path,
                    i32::from(file_ref.is_dir)
                )
                .as_bytes(),
            );
        }
        for image in &payload.images {
            out.extend_from_slice(
                format!(
                    "{}\t{}\t{}\t{}\t{}x{}\t{}\n",
                    image.mime_type,
                    image.blob_id,
                    image.checksum,
                    image.size_bytes,
                    image.width,
                    image.height,
                    image.bytes.len()
                )
                .as_bytes(),
            );
            out.extend_from_slice(&image.bytes);
            out.push(b'\n');
        }
        out
    }

    fn on_native_clipboard_changed(&self) {
        let Some(native) = &self.native else {
            return;
        };
        let Some(payload) = native.read_payload().filter(|payload| !payload.empty()) else {
            return;
        };
        let fingerprint = Self::fingerprint_for(&payload);
        {
            let mut state = self.state.lock().expect("clipboard state lock");
            if fingerprint == state.last_seen_fingerprint {
                return;
            }
            state.last_seen_fingerprint = fingerprint;
        }
        let payload = self.finalize_payload(payload, ClipboardOrigin::LocalSystem);
        self.set_local_payload(payload);
    }

    fn set_local_payload(&self, payload: ClipboardPayload) {
        self.state.lock().expect("clipboard state lock").local = payload;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_changes_with_binary_image_content() {
        let mut first = ClipboardPayload {
            kind: ClipboardPayloadKind::Image,
            images: vec![super::super::ClipboardImage {
                bytes: vec![1, 2, 3],
                ..super::super::ClipboardImage::default()
            }],
            ..ClipboardPayload::default()
        };
        let first_fingerprint = ClipboardService::fingerprint_for(&first);
        first.images[0].bytes[2] = 4;
        assert_ne!(first_fingerprint, ClipboardService::fingerprint_for(&first));
    }

    #[test]
    fn fingerprint_distinguishes_remote_file_ref_provider_metadata() {
        let first = ClipboardPayload {
            kind: ClipboardPayloadKind::FileRefs,
            file_refs: vec![super::super::ClipboardFileRef {
                display_name: "report.pdf".into(),
                provider_type: "drive".into(),
                remote_name: "work".into(),
                remote_path: "/report.pdf".into(),
                ..super::super::ClipboardFileRef::default()
            }],
            ..ClipboardPayload::default()
        };
        let mut second = first.clone();
        second.file_refs[0].provider_type = "onedrive".into();
        let mut third = first.clone();
        third.file_refs[0].display_name = "renamed-report.pdf".into();

        let first_fingerprint = ClipboardService::fingerprint_for(&first);
        assert_ne!(
            first_fingerprint,
            ClipboardService::fingerprint_for(&second)
        );
        assert_ne!(first_fingerprint, ClipboardService::fingerprint_for(&third));
    }

    #[test]
    fn kura_payloads_get_increasing_revisions() {
        let service = ClipboardService::new(None);
        let text = |text: &str| ClipboardPayload {
            kind: ClipboardPayloadKind::Text,
            text: text.to_owned(),
            ..ClipboardPayload::default()
        };
        let first = service.set_local_kura_payload(text("one"));
        let second = service.set_local_kura_payload(text("two"));

        assert_eq!(first.origin, ClipboardOrigin::LocalKura);
        assert_eq!((first.revision, second.revision), (1, 2));
        assert_eq!(service.current_local().text, "two");
    }
}
