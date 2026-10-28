import type { KeyboardEvent } from "react";
import { revealEntry } from "@/features/file-ui/explorer/components/fileBrowser/revealEntry";
import { isMacPlatform } from "@/shared/platform/tauri";
import { useExplorerStore } from "../store";

export const quickLookEvent = "kura:quick-look";

/** The entry keyboard moves start from, per pane (Finder's "lead"). */
const leadByPane = new Map<string, string>();

/**
 * Finder-style keys for a pane's list or grid: arrows (Shift extends),
 * Home/End, Cmd+A, Cmd+Down/Cmd+O to open, Cmd+Up for the parent folder,
 * Return to rename (Enter opens on Windows/Linux) and Space for Quick Look.
 */
export function handleFileListKey(
  event: KeyboardEvent<HTMLElement>,
  paneId: string,
): void {
  if (event.defaultPrevented) return;
  const target = event.target as HTMLElement;
  if (target.closest("input, textarea, select, [contenteditable='true']"))
    return;
  const store = useExplorerStore.getState();
  const pane = store.panes[paneId];
  const entries = pane?.listing?.entries ?? [];
  const selected = pane?.selectedIds ?? [];
  const mac = isMacPlatform();
  const primary = mac ? event.metaKey : event.ctrlKey;
  const key = event.key;
  const selectedEntry = () => entries.find((entry) => entry.id === selected[0]);

  if (primary && !event.shiftKey && !event.altKey) {
    if (key === "ArrowUp") {
      event.preventDefault();
      void store.navigateParent(paneId);
    } else if (key === "ArrowDown" || key.toLowerCase() === "o") {
      const entry = selectedEntry();
      if (!entry || entry.isDeleted) return;
      event.preventDefault();
      void store.openEntry(paneId, entry);
    } else if (key.toLowerCase() === "a" && entries.length > 0) {
      event.preventDefault();
      store.selectEntry(paneId, entries[0].id);
      store.selectEntry(paneId, entries[entries.length - 1].id, {
        range: true,
      });
    }
    return;
  }
  if (event.metaKey || event.ctrlKey || event.altKey) return;

  if (key === " ") {
    const entry = selectedEntry();
    if (!entry || selected.length !== 1) return;
    event.preventDefault();
    window.dispatchEvent(
      new CustomEvent(quickLookEvent, { detail: { path: entry.path } }),
    );
    return;
  }
  if (key === "Enter") {
    const entry = selectedEntry();
    if (!entry || entry.isDeleted) return;
    event.preventDefault();
    if (mac && selected.length === 1) void store.renameSelected(paneId);
    else if (!mac) void store.openEntry(paneId, entry);
    return;
  }

  const grid = event.currentTarget.querySelector<HTMLElement>(
    "[data-grid-columns]",
  );
  const columns = Number(grid?.dataset.gridColumns) || 1;
  const delta =
    key === "ArrowDown"
      ? columns
      : key === "ArrowUp"
        ? -columns
        : key === "ArrowRight" && grid
          ? 1
          : key === "ArrowLeft" && grid
            ? -1
            : key === "Home"
              ? -entries.length
              : key === "End"
                ? entries.length
                : 0;
  if (delta === 0 || entries.length === 0) return;
  event.preventDefault();

  const lead = leadByPane.get(paneId);
  const leadId =
    lead && selected.includes(lead) ? lead : selected[selected.length - 1];
  const leadIndex = leadId
    ? entries.findIndex((entry) => entry.id === leadId)
    : -1;
  const nextIndex =
    leadIndex < 0
      ? delta > 0
        ? 0
        : entries.length - 1
      : Math.min(entries.length - 1, Math.max(0, leadIndex + delta));
  const next = entries[nextIndex];
  store.selectEntry(paneId, next.id, { range: event.shiftKey });
  leadByPane.set(paneId, next.id);
  revealEntry(paneId, next.id);
}
