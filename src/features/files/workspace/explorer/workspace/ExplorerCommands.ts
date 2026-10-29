import type { ExplorerViewMode } from "../model/types/store/types";
import { useOperationQueueStore } from "../store/useOperationQueueStore";
export {
  newestUndoableTransfer,
  transferTypeLabel,
} from "./explorerCommands/transferLabels";
import { useAppStore } from "@/features/app-shell";
import { multiPanelStoreForPane } from "@/features/workspace";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  appOpenWindow,
  operationQueueRedo,
  operationQueueUndo,
  transfersSnapshot,
} from "../../native";
import { errorText } from "@/shared/lib/format";
import {
  selectedEntriesForPane,
  selectedPathsForPane,
  useExplorerStore,
} from "../store";
import {
  goToFolderEvent,
  openInNewTab,
  quickLookEvent,
} from "../components/fileListKeyboard";
export { openInNewTab };
import { openCompareWith } from "./contextMenu/remoteVerification";
import { useSearchStore } from "../../search";
import { toggleActiveTabPanelVisibility } from "./ExplorerDesktopPlugins";
import {
  newestUndoableTransfer,
  transferTypeLabel,
} from "./explorerCommands/transferLabels";

const explorerDuplicateFinderEvent = "kura:explorer-duplicate-finder";
const trashPath = "kura://trash";

export const executableShortcutCommands = [
  "explorer.new_folder",
  "explorer.search",
  "explorer.copy",
  "explorer.cut",
  "explorer.paste",
  "explorer.copy_path",
  "explorer.undo",
  "explorer.redo",
  "explorer.delete",
  "explorer.download",
  "explorer.rename",
  "explorer.batch_rename",
  "explorer.duplicate_finder",
  "explorer.compare_with",
  "explorer.open_with",
  "explorer.refresh",
  "explorer.toggle_hidden",
  "explorer.preview.toggle",
  "explorer.sidebar.toggle",
  "explorer.duplicate",
  "explorer.new_folder_with_selection",
  "explorer.get_info",
  "explorer.quick_look",
  "explorer.go_to_folder",
  "explorer.go_home",
  "explorer.go_desktop",
  "explorer.go_documents",
  "explorer.go_downloads",
  "explorer.go_applications",
  "explorer.go_trash",
  "explorer.empty_trash",
  "explorer.invert_selection",
  "explorer.reveal",
  "explorer.open_in_new_tab",
  "explorer.view_list",
  "explorer.view_grid",
  "explorer.view_columns",
  "explorer.new_window",
] as const;

export function runExplorerCommand(commandId: string, paneId: string): void {
  const explorer = useExplorerStore.getState();
  const multiPanelStore = multiPanelStoreForPane(paneId);
  switch (commandId) {
    case "explorer.new_folder":
      void explorer.createItem(paneId, "folder");
      break;
    case "explorer.search":
      openDeepSearch(paneId);
      break;
    case "explorer.refresh":
      void explorer.refreshPane(paneId);
      break;
    case "explorer.toggle_hidden":
      void explorer.toggleHidden(paneId);
      break;
    case "explorer.rename":
      void explorer.renameSelected(paneId);
      break;
    case "explorer.batch_rename":
      explorer.openBatchRenameDialog(paneId);
      break;
    case "explorer.duplicate_finder":
      openDuplicateFinder(paneId);
      break;
    case "explorer.compare_with":
      openCompareWith(paneId);
      break;
    case "explorer.delete":
      // In the Trash, Finder's Move to Trash key puts items back.
      if (explorer.panes[paneId]?.listing?.path === trashPath)
        void explorer.putBackSelected(paneId);
      else void explorer.deleteSelected(paneId);
      break;
    case "explorer.download":
      void explorer.downloadSelected(paneId);
      break;
    case "explorer.open_with":
      void explorer.openWithSelected(paneId);
      break;
    case "explorer.copy":
      explorer.copySelected(paneId);
      break;
    case "explorer.cut":
      explorer.cutSelected(paneId);
      break;
    case "explorer.paste":
      void explorer.pasteIntoPane(paneId);
      break;
    case "explorer.copy_path": {
      const path = selectedPathsForPane(explorer.panes[paneId])[0];
      if (path) void explorer.copyPath(path);
      break;
    }
    case "explorer.undo":
      void undoLatestTransferOperation();
      break;
    case "explorer.redo":
      void redoLatestTransferOperation();
      break;
    case "explorer.preview.toggle":
      toggleActiveTabPanelVisibility("preview", multiPanelStore);
      break;
    case "explorer.sidebar.toggle":
      toggleActiveTabPanelVisibility("sidebar", multiPanelStore);
      break;
    case "explorer.duplicate":
      void explorer.duplicateSelected(paneId);
      break;
    case "explorer.new_folder_with_selection":
      void explorer.newFolderWithSelection(paneId);
      break;
    case "explorer.get_info":
      toggleActiveTabPanelVisibility("preview", multiPanelStore, true);
      break;
    case "explorer.quick_look":
      quickLookSelected(paneId);
      break;
    case "explorer.go_to_folder":
      window.dispatchEvent(
        new CustomEvent(goToFolderEvent, { detail: { paneId } }),
      );
      break;
    case "explorer.go_home":
    case "explorer.go_desktop":
    case "explorer.go_documents":
    case "explorer.go_downloads":
    case "explorer.go_applications":
    case "explorer.go_trash": {
      const path = standardFolderPath(commandId);
      if (path) void explorer.navigatePane(paneId, path);
      break;
    }
    case "explorer.empty_trash":
      void explorer.emptyTrash(paneId);
      break;
    case "explorer.view_list":
    case "explorer.view_grid":
    case "explorer.view_columns":
      explorer.setViewMode(
        commandId.slice("explorer.view_".length) as ExplorerViewMode,
        paneId,
      );
      break;
    case "explorer.new_window": {
      const path = explorer.panes[paneId]?.listing?.path;
      openWindow(path && !path.startsWith("kura://") ? path : null);
      break;
    }
    case "explorer.invert_selection":
      explorer.invertSelection(paneId);
      break;
    case "explorer.reveal":
      revealSelected(paneId);
      break;
    case "explorer.open_in_new_tab": {
      const folder = selectedEntriesForPane(explorer.panes[paneId]).find(
        (entry) => entry.kind === "folder" && !entry.isDeleted,
      );
      if (folder) openInNewTab(folder.path);
      break;
    }
  }
}

export function quickLookSelected(paneId: string): void {
  const entry = selectedEntriesForPane(
    useExplorerStore.getState().panes[paneId],
  )[0];
  if (entry)
    window.dispatchEvent(
      new CustomEvent(quickLookEvent, { detail: { entry } }),
    );
}

export function revealSelected(paneId: string): void {
  const paths = selectedEntriesForPane(
    useExplorerStore.getState().panes[paneId],
  )
    .filter((entry) => entry.location.kind === "local")
    .map((entry) => entry.path);
  if (paths.length === 0) return;
  revealItemInDir(paths).catch((error: unknown) =>
    useExplorerStore
      .getState()
      .pushNotification(`Could not reveal: ${errorText(error)}`, "error", 4500),
  );
}

export function openWindow(path: string | null): void {
  appOpenWindow(path).catch((error: unknown) =>
    useExplorerStore
      .getState()
      .pushNotification(
        `Could not open a window: ${errorText(error)}`,
        "error",
        4500,
      ),
  );
}

function standardFolderPath(commandId: string): string | null {
  if (commandId === "explorer.go_trash") return trashPath;
  if (commandId === "explorer.go_applications") return "/Applications";
  const home = useAppStore.getState().app?.environment.homeDir;
  if (!home) return null;
  const folder: Record<string, string> = {
    "explorer.go_desktop": "Desktop",
    "explorer.go_documents": "Documents",
    "explorer.go_downloads": "Downloads",
  };
  const name = folder[commandId];
  return name ? `${home.replace(/\/$/, "")}/${name}` : home;
}

function openDuplicateFinder(paneId: string): void {
  window.dispatchEvent(
    new CustomEvent(explorerDuplicateFinderEvent, { detail: { paneId } }),
  );
}

function openDeepSearch(paneId: string): void {
  const currentPath =
    useExplorerStore.getState().panes[paneId]?.listing?.path ?? "";
  void useSearchStore.getState().openSearch(currentPath, paneId);
}

export async function undoLatestTransferOperation(): Promise<void> {
  const explorer = useExplorerStore.getState();
  try {
    const rows = (await transfersSnapshot({ limit: 500 })).rows;
    const latest = newestUndoableTransfer(rows);
    if (!latest) {
      explorer.pushNotification("Nothing to undo.", "info", 3500);
      return;
    }

    const snapshot = await operationQueueUndo(latest.undoTokenId);
    useOperationQueueStore.setState({ snapshot, error: null });
    await useOperationQueueStore.getState().load({ silent: true });
    explorer.pushNotification(
      `Undo queued for ${latest.fileName || transferTypeLabel(latest.transferType)}.`,
      "success",
      3500,
    );
  } catch (error) {
    explorer.pushNotification(
      `Undo failed: ${errorText(error)}`,
      "error",
      4500,
    );
  }
}

export async function redoLatestTransferOperation(): Promise<void> {
  const explorer = useExplorerStore.getState();
  try {
    const snapshot = await operationQueueRedo();
    useOperationQueueStore.setState({ snapshot, error: null });
    await useOperationQueueStore.getState().load({ silent: true });
    explorer.pushNotification("Redo queued.", "success", 3500);
  } catch (error) {
    explorer.pushNotification(
      `Redo failed: ${errorText(error)}`,
      "error",
      4500,
    );
  }
}
