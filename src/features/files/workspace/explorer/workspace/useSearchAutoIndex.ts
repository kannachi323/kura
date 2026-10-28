import {
  selectSearchMaintenancePreferences,
  useSettingsStore,
} from "@/features/settings";
import { hasTauriInternals } from "@/shared/platform/tauri";
import { useEffect } from "react";
import { useShallow } from "zustand/react/shallow";
import { useSearchStore } from "../../search";

/** Leaves launch to the file list; the first scan waits this long. */
const firstScanDelayMs = 30_000;

/**
 * Settings > Search > "Index files automatically": once an index has been
 * built, keeps it current with an incremental scan after launch and then on
 * the chosen interval. Scans are paced and low priority in the backend, which also
 * skips a scan that another one finished within the last minute.
 */
export function useSearchAutoIndex(homePath: string): void {
  const { enabled, intervalMinutes } = useSettingsStore(
    useShallow((state) => {
      const search = selectSearchMaintenancePreferences(
        state.settings?.document,
      );
      return {
        enabled:
          state.settings !== null && search.automaticFileDiscoveryEnabled,
        intervalMinutes: search.discoveryIntervalMinutes,
      };
    }),
  );

  useEffect(() => {
    if (!enabled || !hasTauriInternals()) return;
    const scan = async () => {
      const store = useSearchStore.getState();
      await store.initialize();
      const status = useSearchStore.getState().status;
      // Only keep an existing index current. The first build crawls the
      // whole home folder, so it waits for "Build index" in search.
      if (!status || status.scanInProgress || status.indexedItemCount === 0)
        return;
      await store.startScan(homePath);
    };
    const first = window.setTimeout(() => void scan(), firstScanDelayMs);
    const interval = window.setInterval(
      () => void scan(),
      intervalMinutes * 60_000,
    );
    return () => {
      window.clearTimeout(first);
      window.clearInterval(interval);
    };
  }, [enabled, intervalMinutes, homePath]);
}
