import { useExplorerStore } from "@/features/files/workspace/explorer/store";
import { useSearchStore } from "@/features/files/workspace/search";
import { useMultiPanelStore } from "@/features/workspace";
import { hasTauriInternals } from "@/shared/platform/tauri";
import { getCurrent, onOpenUrl } from "@tauri-apps/plugin-deep-link";
import { useEffect } from "react";

/**
 * Handles `kura://open?path=…&select=…` and `kura://search?q=…`, which other apps
 * (including Misty) use to hand a file or a search to Kura.
 */
export function useDeepLinks(): void {
  useEffect(() => {
    if (!hasTauriInternals()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void getCurrent()
      .then((urls) => urls?.forEach((url) => void handleKuraUrl(url)))
      .catch(() => undefined);
    void onOpenUrl((urls) =>
      urls.forEach((url) => void handleKuraUrl(url)),
    ).then((remove) => {
      if (disposed) remove();
      else unlisten = remove;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);
}

export async function handleKuraUrl(raw: string): Promise<void> {
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    return;
  }
  if (url.protocol !== "kura:") return;
  const action = url.hostname || url.pathname.replace(/^\/+/, "");
  const paneId = await waitForActivePane();
  if (!paneId) return;
  if (action === "open") {
    const path = url.searchParams.get("path");
    if (!path) return;
    const select = url.searchParams.get("select");
    await useExplorerStore.getState().navigatePane(paneId, path);
    if (!select) return;
    const entry = useExplorerStore
      .getState()
      .panes[paneId]?.listing?.entries.find(
        (candidate) => candidate.path === select,
      );
    if (entry) useExplorerStore.getState().selectEntry(paneId, entry.id);
    return;
  }
  if (action === "search") {
    const query = url.searchParams.get("q") ?? "";
    const currentPath =
      useExplorerStore.getState().panes[paneId]?.listing?.path ?? "";
    await useSearchStore.getState().openSearch(currentPath);
    useSearchStore.getState().setQuery(query);
  }
}

async function waitForActivePane(): Promise<string | null> {
  for (let attempt = 0; attempt < 50; attempt += 1) {
    const paneId = useMultiPanelStore.getState().activePaneId;
    if (paneId && useExplorerStore.getState().initialized) return paneId;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  return null;
}
