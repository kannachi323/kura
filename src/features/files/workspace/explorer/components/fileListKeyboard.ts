import type { KeyboardEvent } from "react";
import { revealEntry } from "@/features/file-ui/explorer/components/fileBrowser/revealEntry";
import { isPackageEntry } from "@/features/file-ui";
import { isMacPlatform } from "@/shared/platform/tauri";
import { selectChildOnReturn } from "./ColumnView";
import { useExplorerStore } from "../store";

/** Toggles Quick Look for `detail.entry`. */
export const quickLookEvent = "kura:quick-look";
/** Asks a pane's toolbar to start editing its path ("Go to Folder"). */
export const goToFolderEvent = "kura:go-to-folder";
/** Asks the workspace to open `detail.path` in a new tab. */
export const openInNewTabEvent = "kura:open-in-new-tab";

export function openInNewTab(path: string): void {
  window.dispatchEvent(
    new CustomEvent(openInNewTabEvent, { detail: { path } }),
  );
}

/** The entry keyboard moves start from, per pane (Finder's "lead"). */
const leadByPane = new Map<string, string>();
/** Letters typed in quick succession, per pane, for type-to-select. */
const typedByPane = new Map<string, { text: string; atMs: number }>();
const typeSelectTimeoutMs = 1000;

/**
 * Finder-style keys for a pane's list or grid: arrows (Shift extends),
 * Home/End, Cmd+A, Cmd+Down/Cmd+O to open, Cmd+Up for the parent folder,
 * Return to rename (Enter opens on Windows/Linux), Space for Quick Look and
 * type-to-select.
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

  // Type-to-select: letters jump to the first name that starts with them.
  // Space continues a name being typed rather than opening Quick Look.
  const typed = typedByPane.get(paneId);
  const typing = typed && event.timeStamp - typed.atMs < typeSelectTimeoutMs;
  if (key.length === 1 && (key !== " " || typing)) {
    const text = (typing ? typed.text : "") + key.toLocaleLowerCase();
    typedByPane.set(paneId, { text, atMs: event.timeStamp });
    const match = entries.find((entry) =>
      entry.name.toLocaleLowerCase().startsWith(text),
    );
    event.preventDefault();
    if (!match) return;
    store.selectEntry(paneId, match.id);
    leadByPane.set(paneId, match.id);
    revealEntry(paneId, match.id);
    return;
  }

  if (key === " ") {
    const entry = selectedEntry();
    if (!entry || selected.length !== 1) return;
    event.preventDefault();
    window.dispatchEvent(
      new CustomEvent(quickLookEvent, { detail: { entry } }),
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

  // Column view: Right opens the selected folder, Left goes back up and
  // selects the folder you came from.
  const viewMode = store.paneViewModes[paneId] ?? store.viewMode;
  if (viewMode === "columns" && (key === "ArrowRight" || key === "ArrowLeft")) {
    event.preventDefault();
    if (key === "ArrowRight") {
      const entry = selectedEntry();
      if (entry?.kind === "folder" && !isPackageEntry(entry))
        void store.navigatePane(paneId, entry.path);
    } else if (pane?.listing?.parentPath) {
      selectChildOnReturn(paneId, pane.listing.path);
      void store.navigateParent(paneId);
    }
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
