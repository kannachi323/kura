import { useCloudStore } from "@/features/cloud";
import { listenCloudEvents, type CloudEvent } from "@/native/cloud";
import { useEffect } from "react";
import { useExplorerStore } from "../../store";
import { useOperationQueueStore } from "../../store/useOperationQueueStore";

function samePath(left: string, right: string): boolean {
  const clean = (path: string) => path.replace(/\\/g, "/").replace(/\/+$/, "");
  return clean(left) === clean(right);
}

/**
 * Follows Kuragine: re-lists cloud folders that changed, shows uploads queued
 * for saved files, and keeps the account list current.
 */
export function useCloudEvents() {
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let disposed = false;
    const reloadPanes = (path: string) => {
      const explorer = useExplorerStore.getState();
      for (const [paneId, pane] of Object.entries(explorer.panes)) {
        const shown = pane?.listing?.path;
        if (shown && samePath(shown, path))
          void explorer.loadPane(paneId, shown, "replace");
      }
    };
    const handle = (event: CloudEvent) => {
      switch (event.type) {
        case "listingChanged":
        case "copyUpdated":
          reloadPanes(
            event.type === "listingChanged"
              ? event.path
              : event.path.replace(/[\\/][^\\/]*$/, ""),
          );
          void useOperationQueueStore.getState().load({ silent: true });
          break;
        case "uploadNeeded":
          void useOperationQueueStore.getState().load({ silent: true });
          break;
        case "remotesChanged":
          void useCloudStore.getState().load();
          break;
        case "signInRequired":
          void useCloudStore.getState().load();
          useExplorerStore
            .getState()
            .pushNotification(
              `Sign in to ${event.remote} again in Settings → Cloud.`,
              "error",
              8000,
            );
          break;
      }
    };
    void listenCloudEvents(handle).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);
}
