use std::{path::PathBuf, sync::Arc};

use crate::domain::clipboard::ClipboardService;
#[cfg(desktop)]
use crate::domain::clipboard::NativeClipboard;
use crate::domain::file_sync::FileSyncPairStore;
use crate::infra::{
    devices::DeviceService, directory_size::DirectorySizeService,
    directory_watch::DirectoryWatchService, environment::AppEnvironmentService,
    explorer::ExplorerService, explorer_library::ExplorerLibraryService,
    file_sync::FileSyncService, metadata::MetadataService,
    navigation_names::NavigationNamesService, operation_queue::OperationQueueService,
    power_pack::PowerPackService, search::SearchService, settings::SettingsService,
    transfers::TransferService,
};

/// Every native service Kura's window uses. Nothing here talks to a network.
pub struct KuraRuntime {
    pub navigation_names: NavigationNamesService,
    pub environment: AppEnvironmentService,
    pub clipboard: Arc<ClipboardService>,
    pub transfers: TransferService,
    pub sync_pairs: FileSyncPairStore,
    pub file_sync: FileSyncService,
    pub settings: SettingsService,
    pub devices: DeviceService,
    pub directory_size: DirectorySizeService,
    pub directory_watch: DirectoryWatchService,
    pub metadata: MetadataService,
    pub power_pack: PowerPackService,
    pub search: SearchService,
    pub explorer: ExplorerService,
    pub explorer_library: ExplorerLibraryService,
    pub operation_queue: OperationQueueService,
}

impl KuraRuntime {
    pub fn new() -> Self {
        Self::new_with_data_root(None)
    }

    pub fn new_with_data_root(data_root: Option<PathBuf>) -> Self {
        let environment = AppEnvironmentService::new_with_data_root(data_root);
        crate::infra::data_home::prepare(&environment);
        #[cfg(desktop)]
        let native_clipboard: Arc<dyn NativeClipboard> =
            crate::infra::native_clipboard::SystemClipboardAdapter::new();
        #[cfg(desktop)]
        let clipboard = ClipboardService::new(Some(native_clipboard));
        #[cfg(not(desktop))]
        let clipboard = ClipboardService::new(None);
        #[cfg(desktop)]
        let _ = clipboard.start();
        let transfers = TransferService::new(environment.clone());
        let sync_pairs = FileSyncPairStore::new(environment.kura_db_path());
        let settings = SettingsService::new(environment.clone());
        let devices = DeviceService::new();
        let directory_size = DirectorySizeService::new(environment.clone());
        let metadata = MetadataService::new();
        let explorer_library = ExplorerLibraryService::new(environment.clone());
        let search = SearchService::new(environment.clone());
        let explorer = ExplorerService::new(
            environment.clone(),
            transfers.clone(),
            explorer_library.clone(),
        );
        let operation_queue = OperationQueueService::new(explorer.clone(), transfers.clone());
        let power_pack = PowerPackService::new(
            environment.clone(),
            explorer.clone(),
            operation_queue.clone(),
        );
        let file_sync = FileSyncService::new(
            environment.clone(),
            explorer.clone(),
            operation_queue.clone(),
            sync_pairs.clone(),
        );

        Self {
            navigation_names: NavigationNamesService::new(
                environment.config_dir().join("navigation.json"),
            ),
            environment,
            clipboard,
            transfers,
            sync_pairs,
            file_sync,
            settings,
            devices,
            directory_size,
            directory_watch: DirectoryWatchService::new(),
            metadata,
            power_pack,
            search,
            explorer,
            explorer_library,
            operation_queue,
        }
    }
}
