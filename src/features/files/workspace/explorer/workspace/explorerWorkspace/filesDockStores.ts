import {
  createMultiPanelStore,
  useMultiPanelStore,
  type MultiPanelStoreHook,
} from "@/features/workspace";

const storesByWorkspaceId = new Map<string, MultiPanelStoreHook>();

/** Each Files view in the window owns its own panes and history. */
export function filesMultiPanelStore(
  workspaceId?: string,
): MultiPanelStoreHook {
  if (!workspaceId) return useMultiPanelStore;
  const existing = storesByWorkspaceId.get(workspaceId);
  if (existing) return existing;
  const store = createMultiPanelStore({
    idPrefix: `files-${workspaceId}`,
    defaultTitle: "Files",
  });
  storesByWorkspaceId.set(workspaceId, store);
  return store;
}

export function existingFilesMultiPanelStore(
  workspaceId: string,
): MultiPanelStoreHook | undefined {
  return storesByWorkspaceId.get(workspaceId);
}
