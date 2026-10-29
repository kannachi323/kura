import type { MultiPanelStoreHook, MultiPanelTab } from "@/features/workspace";
import { useMultiPanelStore } from "@/features/workspace";
import type { PluginTabState } from "../../model/types/workspace/ExplorerDesktopPlugins";

export function canCloseExplorerTab(
  tab: MultiPanelTab,
  tabs: MultiPanelTab[],
): boolean {
  return tabs.some((candidate) => candidate.id !== tab.id);
}

export function canOpenTerminalPath(path: string): boolean {
  const trimmed = path.trim();
  return Boolean(trimmed) && !trimmed.includes("://");
}

export function toggleActiveTabPanelVisibility(
  panel: "sidebar" | "preview",
  store: MultiPanelStoreHook = useMultiPanelStore,
  visible?: boolean,
): void {
  const multi = store.getState();
  const activeTab =
    multi.tabs.find((tab) => tab.id === multi.activeTabId) ?? multi.tabs[0];
  if (!activeTab) return;
  if (panel === "sidebar") {
    multi.setTabPanelVisibility(activeTab.id, {
      sidebarVisible: visible ?? !(activeTab.sidebarVisible ?? true),
    });
  } else {
    multi.setTabPanelVisibility(activeTab.id, {
      previewVisible: visible ?? !(activeTab.previewVisible ?? true),
    });
  }
}

export function parsePluginTabPath(path: string): PluginTabState | null {
  if (!path.startsWith("kura-plugin:")) return null;
  try {
    const url = new URL(path);
    const pluginId = url.searchParams.get("plugin") ?? "";
    if (!pluginId) return null;
    return {
      kind: url.hostname === "commands" ? "commands" : "panel",
      pluginId,
      panelId: url.searchParams.get("panel") ?? "",
      selectedPath: url.searchParams.get("selected") ?? "",
    };
  } catch {
    return null;
  }
}
