//! Kura's side of Kuragine: building the engine from Kura's folders and
//! reporting cloud transfers in the transfer history.
use std::{
    sync::{atomic::AtomicBool, atomic::Ordering, Arc, Mutex},
    time::Instant,
};

use kuragine::{KeychainStore, Kuragine, KuragineConfig, TransferObserver};

use crate::infra::{environment::AppEnvironmentService, transfers::TransferService};

/// The engine for `~/.kura/providers`, with its index in `~/.kura/db`.
pub fn engine(environment: &AppEnvironmentService) -> Kuragine {
    let config = || KuragineConfig {
        providers_root: environment.mount_root(),
        conflicts_dir: environment.cache_dir().join("cloud-conflicts"),
        db_path: environment.db_dir().join("kuragine.db"),
        secrets: Arc::new(KeychainStore),
    };
    Kuragine::new(config()).unwrap_or_else(|error| {
        eprintln!("kura: cloud index unavailable, keeping it in memory: {error}");
        Kuragine::in_memory(config()).expect("in-memory cloud index")
    })
}

/// Writes a cloud transfer's progress to its transfer row (at most every
/// 200 ms) and passes on cancellation from the operation queue.
pub struct TransferProgress<'a> {
    transfers: TransferService,
    transfer_id: Option<u64>,
    cancellation: Option<&'a AtomicBool>,
    started: Instant,
    last_report: Mutex<Option<Instant>>,
}

impl<'a> TransferProgress<'a> {
    pub fn new(
        transfers: TransferService,
        transfer_id: Option<u64>,
        cancellation: Option<&'a AtomicBool>,
    ) -> Self {
        Self {
            transfers,
            transfer_id,
            cancellation,
            started: Instant::now(),
            last_report: Mutex::new(None),
        }
    }
}

impl TransferObserver for TransferProgress<'_> {
    fn progress(&self, done: u64, total: u64) {
        let Some(id) = self.transfer_id else {
            return;
        };
        {
            let mut last = self.last_report.lock().unwrap();
            let finished = done >= total;
            if last.is_some_and(|last| last.elapsed().as_millis() < 200) && !finished {
                return;
            }
            *last = Some(Instant::now());
        }
        let speed = done as f64 / self.started.elapsed().as_secs_f64().max(0.001);
        let transfers = self.transfers.clone();
        let done = done.min(i64::MAX as u64) as i64;
        let total = total.min(i64::MAX as u64) as i64;
        tokio::spawn(async move {
            let _ = transfers
                .update_progress_with_speed(id, done, total.max(done), speed)
                .await;
        });
    }

    fn is_canceled(&self) -> bool {
        self.cancellation
            .is_some_and(|cancellation| cancellation.load(Ordering::SeqCst))
    }
}
