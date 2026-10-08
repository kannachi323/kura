import "../../../native";
import {
  registerShortcutHandler,
  shortcutCommandsById,
} from "@/features/shortcuts";
import {
  useMultiPanelStore,
  type MultiPanelStoreHook,
} from "@/features/workspace";
import { useEffect, type RefObject } from "react";
import { useExplorerStore } from "../../store";
import { runExplorerCommand } from "../ExplorerCommands";
import { parsePluginTabPath } from "../ExplorerDesktopPlugins";
import { transfersPath } from "../../../../transfers/transferModel";

const tabCommandIds = new Set([
  "explorer.new_tab",
  "explorer.close_tab",
  "explorer.restore_tab",
  "explorer.next_workspace",
  ...Array.from({ length: 9 }, (_, index) => `explorer.tab_${index + 1}`),
]);
export function useLegacyPluginTabMigration(options: {
  homePath: string;
  workspacePathSignature: string;
  multiPanelStore?: MultiPanelStoreHook;
}): void {
  const { homePath, workspacePathSignature } = options;
  useEffect(() => {
    if (!homePath) return;
    const multi = (options.multiPanelStore ?? useMultiPanelStore).getState();
    const legacyTabs = multi.tabs.filter((tab) => parsePluginTabPath(tab.path));
    for (const tab of legacyTabs) {
      multi.updateActiveTabPath(tab.activePaneId, homePath, "Files");
      multi.setTabPanelVisibility(tab.id, {
        sidebarVisible: true,
        previewVisible: true,
      });
    }
  }, [homePath, options.multiPanelStore, workspacePathSignature]);
}
export function useOperationErrorNotification(
  operationError: string | null,
  pushNotification: (
    message: string,
    type?: "info" | "error" | "success",
    duration?: number,
  ) => void,
): void {
  useEffect(() => {
    if (!operationError) return;
    const message = useExplorerStore.getState().consumeOperationError();
    if (!message) return;
    const recoveredWorkspace =
      message.startsWith("Kura reset a damaged Explorer profile") ||
      message.startsWith("Profile could not be restored");
    pushNotification(message, recoveredWorkspace ? "info" : "error", 4500);
  }, [operationError, pushNotification]);
}
export function useExplorerKeyboardShortcuts(options: {
  active?: boolean;
  executableCommandIdsRef: RefObject<readonly string[]>;
  multiPanelStore?: MultiPanelStoreHook;
}): void {
  const { executableCommandIdsRef } = options;
  useEffect(() => {
    const multiPanelStore = options.multiPanelStore ?? useMultiPanelStore;
    const enabled = () => options.active !== false;
    const onKeyDown = (event: KeyboardEvent) => {
      if (!enabled()) return;
      const target = event.target as HTMLElement | null;
      const editing =
        target?.matches("input, textarea, select, [contenteditable='true']") ??
        false;
      const explorerState = useExplorerStore.getState();
      const paneId = multiPanelStore.getState().activePaneId;
      if (!paneId) return;
      if (event.key === "Escape") {
        explorerState.cancelInlineEdit();
        explorerState.closeContextMenu();
        return;
      }
      if (editing) return;
    };
    window.addEventListener("keydown", onKeyDown);
    const run = (commandId: string) => {
      const multi = multiPanelStore.getState();
      if (
        multi.tabs.find((tab) => tab.id === multi.activeTabId)?.path ===
          transfersPath &&
        commandId.startsWith("explorer.") &&
        !tabCommandIds.has(commandId)
      )
        return;
      const paneId = multiPanelStore.getState().activePaneId;
      if (!paneId) return;
      runExplorerCommand(commandId, paneId);
    };
    const commandIds = [...executableCommandIdsRef.current].filter(
      (commandId) => shortcutCommandsById.has(commandId),
    );
    const unregister = commandIds.map((commandId) =>
      registerShortcutHandler(commandId, () => run(commandId), enabled),
    );
    unregister.push(
      registerShortcutHandler(
        "navigation.back",
        () => {
          const paneId = multiPanelStore.getState().activePaneId;
          if (
            !paneId ||
            !useExplorerStore.getState().panes[paneId]?.backHistory.length
          )
            return false;
          void useExplorerStore.getState().navigateBack(paneId);
          return true;
        },
        enabled,
        100,
      ),
      registerShortcutHandler(
        "navigation.forward",
        () => {
          const paneId = multiPanelStore.getState().activePaneId;
          if (
            !paneId ||
            !useExplorerStore.getState().panes[paneId]?.forwardHistory.length
          )
            return false;
          void useExplorerStore.getState().navigateForward(paneId);
          return true;
        },
        enabled,
        100,
      ),
    );
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      unregister.forEach((remove) => remove());
    };
  }, [executableCommandIdsRef, options.multiPanelStore, options.active]);
}
