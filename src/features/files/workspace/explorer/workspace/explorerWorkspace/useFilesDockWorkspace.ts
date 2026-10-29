import {
  allLayoutViews,
  useWorkspaceStore,
  type MultiPanelStoreHook,
} from "@/features/workspace";
import { useCallback, useEffect } from "react";

/**
 * Ties an embedded Files view to its window tab: the tab opens on the folder it
 * last showed, and is named after the folder it shows now.
 */
export function useFilesDockWorkspace(options: {
  workspaceId?: string;
  activePath: string;
  initialized: boolean;
  homePath: string;
  multiPanelStore: MultiPanelStoreHook;
}) {
  const { workspaceId, activePath, initialized, homePath, multiPanelStore } =
    options;

  useEffect(() => {
    if (!initialized || !homePath) return;
    const multi = multiPanelStore.getState();
    if (multi.tabs.length) return;
    const saved = workspaceId
      ? allLayoutViews(useWorkspaceStore.getState().layout).find(
          (view) => view.id === workspaceId,
        )?.state.path
      : undefined;
    multi.initialize(saved || homePath, "Files");
  }, [homePath, initialized, multiPanelStore, workspaceId]);

  useEffect(() => {
    if (!workspaceId || !activePath) return;
    const view = allLayoutViews(useWorkspaceStore.getState().layout).find(
      (candidate) => candidate.id === workspaceId,
    );
    if (!view || view.state.path === activePath) return;
    useWorkspaceStore
      .getState()
      .updateViewState(
        workspaceId,
        { path: activePath },
        folderTitle(activePath),
      );
  }, [activePath, workspaceId]);

  return useCallback((path: string, title?: string) => {
    const workspace = useWorkspaceStore.getState();
    const view = workspace.openSurface({
      surfaceId: "files",
      title: title || folderTitle(path),
      state: { path },
      forceNew: true,
    });
    workspace.focusView(view.id);
  }, []);
}

export function folderTitle(path: string): string {
  const normalized = path.replace(/[\\/]+$/, "");
  return normalized.split(/[\\/]/).filter(Boolean).pop() ?? (normalized || "/");
}
