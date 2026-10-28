import { useOperationQueueStore } from "../store/useOperationQueueStore";
export {
  newestUndoableTransfer,
  transferTypeLabel,
} from "./explorerCommands/transferLabels";
import { multiPanelStoreForPane } from "@/features/workspace";
import {
  operationQueueRedo,
  operationQueueUndo,
  transfersSnapshot,
} from "../../native";
import { errorText } from "@/shared/lib/format";
import { selectedPathsForPane, useExplorerStore } from "../store";
import { openCompareWith } from "./ExplorerContextMenu";
import { useSearchStore } from "../../search";
import { toggleActiveTabPanelVisibility } from "./ExplorerDesktopPlugins";
import {
  newestUndoableTransfer,
  transferTypeLabel,
} from "./explorerCommands/transferLabels";

const explorerDuplicateFinderEvent = "kura:explorer-duplicate-finder";

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
      void explorer.deleteSelected(paneId);
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
  }
}

function openDuplicateFinder(paneId: string): void {
  window.dispatchEvent(
    new CustomEvent(explorerDuplicateFinderEvent, { detail: { paneId } }),
  );
}

function openDeepSearch(paneId: string): void {
  const currentPath =
    useExplorerStore.getState().panes[paneId]?.listing?.path ?? "";
  void useSearchStore.getState().openSearch(currentPath);
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
