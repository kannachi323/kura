import type {
  DirectoryListing,
  DirectorySizeRecord,
  FileEntry,
} from "@/native/ipc";
import type { MouseEvent } from "react";
import type {
  ExplorerCommandQueryMode,
  ExplorerInlineEditState,
  ExplorerSortColumn,
  ExplorerSortState,
  ExplorerViewMode,
} from "../../../store/index";
import type { ExplorerDragModifiers, ExplorerDragPayload } from "../drag/types";

import type { GridThumbnailSubscriber } from "../../types/components/FileBrowser";

export interface FileBrowserProps {
  paneId: string;
  selectionOnly?: boolean;
  listing: DirectoryListing | null;
  selectedIds: string[];
  loading: boolean;
  error: string | null;
  viewMode: ExplorerViewMode;
  itemScale?: number;
  sort: ExplorerSortState;
  showHidden: boolean;
  commandQuery: string;
  commandQueryMode: ExplorerCommandQueryMode;
  directorySizes: Record<string, DirectorySizeRecord>;
  cutPaths: ReadonlySet<string>;
  inlineEdit: ExplorerInlineEditState | null;
  onSort: (column: ExplorerSortColumn) => void;
  onToggleHidden: () => void;
  onSelect: (
    entryId: string,
    event: MouseEvent,
    visibleEntryIds: string[],
  ) => void;
  onClearSelection: () => void;
  /** Shown at the right of the status bar, e.g. "245 GB available". */
  statusDetail?: string | null;
  /** Replaces the selection; used by rubber-band selection. */
  onSelectIds?: (entryIds: string[]) => void;
  /** `newTab`: Cmd+double-click or middle-click on a folder. */
  onOpen: (entry: FileEntry, options?: { newTab?: boolean }) => void;
  onContextMenu: (event: MouseEvent, entry: FileEntry) => void;
  onBackgroundContextMenu: (event: MouseEvent) => void;
  onDropItems: (
    payload: ExplorerDragPayload,
    destination: string,
    destinationStorageId: string,
    modifiers: ExplorerDragModifiers,
  ) => void;
  onInlineEditChange: (value: string) => void;
  onInlineEditCommit: () => void;
  onInlineEditCancel: () => void;
}

export interface GridThumbnailJob {
  key: string;
  entry: FileEntry;
  maxDimension: number;
  subscribers: Set<GridThumbnailSubscriber>;
  processing: boolean;
  background: boolean;
}
