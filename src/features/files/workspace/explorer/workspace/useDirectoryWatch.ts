import { explorerWatchDirectories } from "@/native/filesystem";
import { hasTauriInternals } from "@/shared/platform/tauri";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { useExplorerStore } from "../store";

const directoryChangedEvent = "kura://directory-changed";

/**
 * Keeps open folders current when other apps change them: the backend
 * watches every folder shown in a pane and reports changed ones, which are
 * reloaded quietly (no skeleton, selection kept).
 */
export function useDirectoryWatch(): void {
  const watchedKey = useExplorerStore((state) =>
    [
      ...new Set(
        Object.values(state.panes)
          .map((pane) => pane.listing?.path)
          .filter((path): path is string => !!path && !path.includes("://")),
      ),
    ]
      .sort()
      .join("\n"),
  );

  useEffect(() => {
    if (!hasTauriInternals()) return;
    void explorerWatchDirectories(
      watchedKey ? watchedKey.split("\n") : [],
    ).catch(() => {});
  }, [watchedKey]);

  useEffect(() => {
    if (!hasTauriInternals()) return;
    let active = true;
    let unlisten: UnlistenFn | null = null;
    void listen<string[]>(directoryChangedEvent, (event) => {
      const changed = new Set(event.payload);
      const store = useExplorerStore.getState();
      for (const [paneId, pane] of Object.entries(store.panes)) {
        const path = pane.listing?.path;
        if (path && changed.has(path) && !pane.loading) {
          void store.loadPane(paneId, path, "replace");
        }
      }
    })
      .then((stop) => {
        if (active) unlisten = stop;
        else void stop();
      })
      .catch(() => {});
    return () => {
      active = false;
      if (unlisten) void unlisten();
    };
  }, []);
}
