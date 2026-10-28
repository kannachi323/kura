import type {
  AnalysisResult,
  CreateItemRequest,
  DeleteItemsRequest,
  DeviceSnapshot,
  DirectoryListing,
  DirectorySizeRecord,
  DirectorySizeRequest,
  ExplorerLibraryItem,
  ExplorerLibrarySnapshot,
  ExplorerOperationResult,
  ExplorerPreviewPayload,
  FileMetadataSnapshot,
  FolderLibraryStatus,
  GeneratedImageThumbnail,
  ListDirectoryRequest,
  MediaSearchSnapshot,
  OperationQueueSnapshot,
  PasteBlobRequest,
  PasteItemsRequest,
  PasteTextRequest,
  PreparedDragItemsResult,
  PreparedMediaChunk,
  PreparedOpenItem,
  PrepareDragItemsRequest,
  PreparedSmartLibraryPreview,
  PrepareOpenItemRequest,
  RenameItemRequest,
  RenameItemsRequest,
  ResolvedMediaAsset,
  ResolvedSmartLibraryAsset,
  SavePreviewRequest,
  SearchQueryRequest,
  SearchResult,
  SearchScanRequest,
  SearchStatus,
  SmartLibraryAssetsPage,
  SmartLibraryAssetsPageRequest,
  SmartLibraryImportPreflight,
  SmartLibraryImportResult,
  SmartLibrarySnapshot,
} from "@/native/ipc";
import { invoke } from "@tauri-apps/api/core";

// Host filesystem services shared by scoped workers and native selection.
// Downloaded apps own their workspace behavior and presentation.
export * from "@/native/runtime";
export * from "@/native/settings-plugins";
export * from "@/native/transfers-tools";
export function devicesSnapshot(): Promise<DeviceSnapshot> {
  return invoke("devices_snapshot");
}
export function devicesUnmount(request: {
  volumeId: string;
  mountPath: string;
}): Promise<DeviceSnapshot> {
  return invoke("devices_unmount", {
    request,
  });
}
export function explorerListDirectory(
  request: ListDirectoryRequest,
): Promise<DirectoryListing> {
  return invoke("explorer_list_directory", {
    request,
  });
}
/** Replaces the set of folders watched for outside changes. */
export function explorerWatchDirectories(paths: string[]): Promise<void> {
  return invoke("explorer_watch_directories", { paths });
}
export function explorerDirectorySizeSnapshot(
  paths: string[],
): Promise<DirectorySizeRecord[]> {
  return invoke("explorer_directory_size_snapshot", {
    paths,
  });
}
export function explorerCalculateDirectorySizes(
  request: DirectorySizeRequest,
): Promise<DirectorySizeRecord[]> {
  return invoke("explorer_calculate_directory_sizes", {
    request,
  });
}
function invokeSearch<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  return invoke<T>(command, args);
}
export function searchInit(): Promise<SearchStatus> {
  return invokeSearch("search_init");
}
export function searchGetStatus(): Promise<SearchStatus> {
  return invokeSearch("search_get_status");
}
export function searchStartScan(
  request: SearchScanRequest,
): Promise<SearchStatus> {
  return invokeSearch("search_start_scan", {
    request,
  });
}
export function searchCancelScan(): Promise<SearchStatus> {
  return invokeSearch("search_cancel_scan");
}
export function searchQuery(
  request: SearchQueryRequest,
): Promise<SearchResult[]> {
  return invokeSearch("search_query", {
    request,
  });
}
export function explorerPrepareOpenItem(
  request: PrepareOpenItemRequest,
): Promise<PreparedOpenItem> {
  return invoke("explorer_prepare_open_item", {
    request,
  });
}
export function explorerPrepareDragItems(
  request: PrepareDragItemsRequest,
): Promise<PreparedDragItemsResult> {
  return invoke("explorer_prepare_drag_items", {
    request,
  });
}
export function explorerCancelDragPreparation(
  sessionId: string,
): Promise<void> {
  return invoke("explorer_cancel_drag_preparation", {
    sessionId,
  });
}
export function explorerPreviewItem(
  path: string,
): Promise<ExplorerPreviewPayload> {
  return invoke("explorer_preview_item", {
    path,
  });
}
export function explorerSavePreviewItem(
  request: SavePreviewRequest,
): Promise<ExplorerOperationResult> {
  return invoke("explorer_save_preview_item", {
    request,
  });
}
export function explorerGenerateImageThumbnail(
  path: string,
  maxDimension: number,
  options: {
    modifiedMs?: number | null;
    remoteModified?: string | null;
    sizeBytes?: number | null;
  } = {},
): Promise<GeneratedImageThumbnail> {
  const args = {
    path,
    maxDimension,
    modifiedMs: options.modifiedMs ?? null,
    remoteModified: options.remoteModified ?? null,
    sizeBytes: options.sizeBytes ?? null,
  };
  return invoke("explorer_generate_image_thumbnail", args);
}
export function fileMetadataSnapshot(
  path: string,
): Promise<FileMetadataSnapshot> {
  return invoke("file_metadata_snapshot", {
    path,
  });
}
export function explorerPathIsDirectory(path: string): Promise<boolean> {
  return invoke("explorer_path_is_directory", {
    path,
  });
}
export function explorerPathExists(path: string): Promise<boolean> {
  return invoke("explorer_path_exists", {
    path,
  });
}
export function explorerLibrarySnapshot(): Promise<ExplorerLibrarySnapshot> {
  return invoke("explorer_library_snapshot");
}
export function explorerLibraryRecordRecent(
  item: ExplorerLibraryItem,
): Promise<ExplorerLibrarySnapshot> {
  return invoke("explorer_library_record_recent", {
    request: {
      item,
    },
  });
}
export function explorerLibraryRecordLastOpened(
  path: string,
): Promise<ExplorerLibrarySnapshot> {
  return invoke("explorer_library_record_last_opened", {
    request: {
      path,
    },
  });
}
export function explorerOpenWith(
  applicationPath: string,
  filePath: string,
): Promise<void> {
  return invoke("explorer_open_with", {
    applicationPath,
    filePath,
  });
}
export function explorerOpenPath(filePath: string): Promise<void> {
  return invoke("explorer_open_path", {
    filePath,
  });
}
export function openTerminalAtPath(path: string): Promise<void> {
  return invoke("open_terminal_at_path", {
    path,
  });
}
export function explorerOpenAssociation(
  filePath: string,
): Promise<string | null> {
  return invoke("explorer_open_association", {
    filePath,
  });
}
export function explorerQueuePasteItems(
  request: PasteItemsRequest,
): Promise<OperationQueueSnapshot> {
  return invoke("explorer_queue_paste_items", {
    request,
  });
}
export function explorerQueuePasteText(
  request: PasteTextRequest,
): Promise<OperationQueueSnapshot> {
  return invoke("explorer_queue_paste_text", {
    request,
  });
}
export function explorerQueuePasteBlob(
  request: PasteBlobRequest,
): Promise<OperationQueueSnapshot> {
  return invoke("explorer_queue_paste_blob", {
    request,
  });
}
export function explorerQueueCreateItem(
  request: CreateItemRequest,
): Promise<OperationQueueSnapshot> {
  return invoke("explorer_queue_create_item", {
    request,
  });
}
export function explorerQueueRenameItem(
  request: RenameItemRequest,
): Promise<OperationQueueSnapshot> {
  return invoke("explorer_queue_rename_item", {
    request,
  });
}
export function explorerQueueRenameItems(
  request: RenameItemsRequest,
): Promise<OperationQueueSnapshot> {
  return invoke("explorer_queue_rename_items", {
    request,
  });
}
export function explorerQueueDeleteItems(
  request: DeleteItemsRequest,
): Promise<OperationQueueSnapshot> {
  return invoke("explorer_queue_delete_items", {
    request,
  });
}
