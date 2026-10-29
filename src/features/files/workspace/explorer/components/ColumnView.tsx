import {
  dragItemsForEntry,
  FileIcon,
  isPackageEntry,
  kindLabel,
  storageIdForPath,
  transferDropAcceptance,
  useExplorerDragSource,
  useExplorerDropZone,
  type ExplorerDragModifiers,
  type ExplorerDragPayload,
  type FileBrowserProps,
} from "@/features/file-ui";
import { InlineNameEditor } from "@/features/file-ui/explorer/components/FileBrowserInline";
import type { DirectoryListing, FileEntry } from "@/native/ipc";
import { ChevronRight } from "lucide-react";
import {
  memo,
  useEffect,
  useMemo,
  useLayoutEffect,
  useRef,
  useState,
  type MouseEvent,
} from "react";
import { explorerListDirectory } from "../../native";
import { explorerRuntime } from "../store/runtime";
import { useExplorerStore, type ExplorerInlineEditState } from "../store";

const COLUMN_WIDTH = 240;
const ROW_HEIGHT = 26;
const OVERSCAN = 8;

/**
 * Finder's column view. The last column is the pane's folder and uses the
 * pane's selection, so the inspector, keyboard and commands work as in the
 * other views. Columns to its left are the folders above it, from Home (or
 * the volume root), each with the next folder highlighted.
 */
export const ColumnView = memo(function ColumnView(props: {
  paneId: string;
  listing: DirectoryListing;
  homePath: string;
  showHidden: boolean;
  selectedIds: string[];
  inlineEdit: ExplorerInlineEditState | null;
  statusDetail: string | null;
  onContextMenu: (event: MouseEvent, entry: FileEntry | null) => void;
  onDropItems: FileBrowserProps["onDropItems"];
}) {
  const ancestors = ancestorPaths(props.listing.path, props.homePath);
  const parentListings = useAncestorListings(
    ancestors,
    props.showHidden,
    props.listing,
  );
  const scrollRef = useRef<HTMLDivElement | null>(null);
  // Keep the current folder's column in view.
  useLayoutEffect(() => {
    const element = scrollRef.current;
    if (element) element.scrollLeft = element.scrollWidth;
  }, [props.listing.path, ancestors.length]);

  const store = useExplorerStore.getState;
  const openFolder = (entry: FileEntry) => {
    void store().navigatePane(props.paneId, entry.path);
  };
  const clickAncestorEntry = (entry: FileEntry) => {
    if (entry.kind === "folder" && !isPackageEntry(entry)) {
      openFolder(entry);
      return;
    }
    // A file in an earlier column: show its folder and select it.
    void store().showInEnclosingFolder(props.paneId, entry);
  };
  const clickCurrentEntry = (entry: FileEntry, event: MouseEvent) => {
    const range = event.shiftKey;
    const toggle = event.metaKey || event.ctrlKey;
    store().selectEntry(props.paneId, entry.id, { range, toggle });
    if (!range && !toggle && entry.kind === "folder" && !isPackageEntry(entry))
      openFolder(entry);
  };
  const selected = new Set(props.selectedIds);
  const selectedCount = props.selectedIds.length;

  return (
    <section className="flex h-full min-h-0 min-w-0 flex-col">
      <div
        ref={scrollRef}
        className="kura-transient-scrollbar flex min-h-0 flex-1 overflow-x-auto overflow-y-hidden"
        onContextMenu={(event) => {
          if (event.target === event.currentTarget)
            props.onContextMenu(event, null);
        }}
      >
        {ancestors.map((path, index) => {
          const childPath = ancestors[index + 1] ?? props.listing.path;
          return (
            <Column
              key={path}
              folderPath={path}
              onDropItems={props.onDropItems}
              entries={parentListings[path] ?? null}
              highlightedPath={childPath}
              dimHighlight
              onEntryClick={(entry) => clickAncestorEntry(entry)}
              onEntryDoubleClick={(entry) =>
                void store().openEntry(props.paneId, entry)
              }
            />
          );
        })}
        <Column
          key={props.listing.path}
          folderPath={props.listing.path}
          onDropItems={props.onDropItems}
          entries={props.listing.entries}
          selected={selected}
          paneId={props.paneId}
          inlineEdit={
            props.inlineEdit?.paneId === props.paneId ? props.inlineEdit : null
          }
          onEntryClick={clickCurrentEntry}
          onEntryDoubleClick={(entry) =>
            void store().openEntry(props.paneId, entry)
          }
          onEntryContextMenu={(event, entry) =>
            props.onContextMenu(event, entry)
          }
          onBackgroundClick={() => store().clearSelection(props.paneId)}
          onBackgroundContextMenu={(event) => props.onContextMenu(event, null)}
        />
      </div>
      <footer className="flex min-h-9 items-center justify-between gap-3 px-3 py-1.5 text-xs text-cream-muted">
        <span className="truncate">
          {selectedCount > 0
            ? `${selectedCount} selected`
            : `${props.listing.totalCount} ${props.listing.totalCount === 1 ? "item" : "items"}`}
        </span>
        {props.statusDetail ? (
          <span className="truncate">{props.statusDetail}</span>
        ) : null}
      </footer>
    </section>
  );
});

function Column(props: {
  folderPath: string;
  entries: FileEntry[] | null;
  onDropItems: FileBrowserProps["onDropItems"];
  highlightedPath?: string;
  dimHighlight?: boolean;
  selected?: Set<string>;
  paneId?: string;
  inlineEdit?: ExplorerInlineEditState | null;
  onEntryClick: (entry: FileEntry, event: MouseEvent) => void;
  onEntryDoubleClick: (entry: FileEntry) => void;
  onEntryContextMenu?: (event: MouseEvent, entry: FileEntry) => void;
  onBackgroundClick?: () => void;
  onBackgroundContextMenu?: (event: MouseEvent) => void;
}) {
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const [scrollTop, setScrollTop] = useState(0);
  // Dropping on a column's empty space drops into that column's folder.
  const { folderPath, onDropItems } = props;
  const dropSpec = useMemo(
    () => ({
      id: `column:${folderPath}`,
      priority: 10,
      accepts: (payload: ExplorerDragPayload) =>
        transferDropAcceptance(payload, folderPath, { folder: true }),
      onDrop: (
        payload: ExplorerDragPayload,
        modifiers: ExplorerDragModifiers,
      ) =>
        onDropItems(
          payload,
          folderPath,
          storageIdForPath(folderPath, null),
          modifiers,
        ),
    }),
    [folderPath, onDropItems],
  );
  const drop = useExplorerDropZone(dropSpec);
  const setScrollElement = (element: HTMLDivElement | null) => {
    scrollRef.current = element;
    drop.ref(element);
  };
  const [height, setHeight] = useState(600);
  const entries = props.entries ?? [];
  useLayoutEffect(() => {
    const element = scrollRef.current;
    if (!element) return;
    const observer = new ResizeObserver(() => setHeight(element.clientHeight));
    observer.observe(element);
    setHeight(element.clientHeight);
    return () => observer.disconnect();
  }, []);
  // Opening a folder replaces the current column; keep keyboard focus.
  const isCurrent = Boolean(props.selected);
  useEffect(() => {
    const active = document.activeElement;
    if (isCurrent && (!active || active === document.body))
      scrollRef.current?.focus({ preventScroll: true });
  }, [isCurrent]);
  // Bring the highlighted or first selected row into view.
  const focusIndex = entries.findIndex((entry) =>
    props.highlightedPath
      ? entry.path === props.highlightedPath
      : props.selected?.has(entry.id),
  );
  useLayoutEffect(() => {
    const element = scrollRef.current;
    if (!element || focusIndex < 0) return;
    const top = focusIndex * ROW_HEIGHT;
    if (top < element.scrollTop) element.scrollTop = top;
    else if (top + ROW_HEIGHT > element.scrollTop + element.clientHeight)
      element.scrollTop = top + ROW_HEIGHT - element.clientHeight;
  }, [focusIndex]);

  const creating = props.inlineEdit?.kind === "create";
  const first = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN);
  const last = Math.min(
    entries.length,
    Math.ceil((scrollTop + height) / ROW_HEIGHT) + OVERSCAN,
  );
  return (
    <div
      ref={setScrollElement}
      role="listbox"
      aria-multiselectable={props.selected ? true : undefined}
      // The current column takes focus so the pane's keys (arrows, Return,
      // Space, type-to-select) work as in the other views.
      tabIndex={props.selected ? 0 : undefined}
      className="kura-transient-scrollbar relative h-full flex-none overflow-y-auto border-r border-charcoal-border py-1 outline-none"
      style={{ width: COLUMN_WIDTH }}
      onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
      onClick={(event) => {
        if (event.target === event.currentTarget) props.onBackgroundClick?.();
      }}
      onContextMenu={(event) => {
        if (event.target !== event.currentTarget) return;
        event.preventDefault();
        props.onBackgroundContextMenu?.(event);
      }}
    >
      {props.entries === null ? (
        <p className="px-3 py-2 text-xs text-cream-muted">Loading…</p>
      ) : entries.length === 0 && !creating ? (
        <p className="px-3 py-2 text-xs text-cream-muted">Empty folder</p>
      ) : null}
      {creating && props.inlineEdit ? (
        <div className="flex h-[26px] items-center gap-2 px-2">
          <InlineNameEditor
            edit={props.inlineEdit}
            variant="table"
            onChange={useExplorerStore.getState().setInlineEditValue}
            onCommit={() => void useExplorerStore.getState().commitInlineEdit()}
            onCancel={useExplorerStore.getState().cancelInlineEdit}
          />
        </div>
      ) : null}
      <div
        style={{ height: entries.length * ROW_HEIGHT, position: "relative" }}
      >
        {entries.slice(first, last).map((entry, offset) => (
          <ColumnRow
            key={entry.id}
            entry={entry}
            top={(first + offset) * ROW_HEIGHT}
            entries={entries}
            selected={props.selected}
            highlighted={entry.path === props.highlightedPath}
            dimHighlight={props.dimHighlight}
            inlineEdit={
              props.inlineEdit?.kind === "rename" &&
              props.inlineEdit.entryId === entry.id
                ? props.inlineEdit
                : null
            }
            onDropItems={props.onDropItems}
            onClick={(event) => {
              if (props.selected)
                scrollRef.current?.focus({ preventScroll: true });
              props.onEntryClick(entry, event);
            }}
            onDoubleClick={() => props.onEntryDoubleClick(entry)}
            onContextMenu={
              props.onEntryContextMenu
                ? (event) => props.onEntryContextMenu?.(event, entry)
                : undefined
            }
          />
        ))}
      </div>
    </div>
  );
}

/** One row: a drag source, and a drop target when it is a folder. */
function ColumnRow(props: {
  entry: FileEntry;
  top: number;
  entries: FileEntry[];
  selected?: Set<string>;
  highlighted: boolean;
  dimHighlight?: boolean;
  inlineEdit: ExplorerInlineEditState | null;
  onDropItems: FileBrowserProps["onDropItems"];
  onClick: (event: MouseEvent) => void;
  onDoubleClick: () => void;
  onContextMenu?: (event: MouseEvent) => void;
}) {
  const { entry } = props;
  const isSelected = props.selected?.has(entry.id) ?? false;
  const folder = entry.kind === "folder" && !isPackageEntry(entry);
  // Earlier columns have no selection; dragging there drags just that row.
  const dragSelection = props.selected ?? new Set([entry.id]);
  const source = useExplorerDragSource(
    entry.isDeleted
      ? []
      : dragItemsForEntry(entry, props.entries, dragSelection),
  );
  const dropSpec = useMemo(
    () => ({
      id: `column-entry:${entry.id}`,
      priority: 20,
      accepts: (payload: ExplorerDragPayload) =>
        transferDropAcceptance(payload, entry.path, {
          folder,
          writable: !entry.readonly && !entry.isDeleted,
        }),
      onDrop: (
        payload: ExplorerDragPayload,
        modifiers: ExplorerDragModifiers,
      ) =>
        props.onDropItems(
          payload,
          entry.path,
          storageIdForPath(entry.path, entry.location.remoteName),
          modifiers,
        ),
      springLoad: folder,
    }),
    [entry, folder, props],
  );
  const drop = useExplorerDropZone(dropSpec);
  return (
    <div
      ref={drop.ref}
      role="option"
      aria-selected={isSelected || props.highlighted}
      data-entry-id={props.selected ? entry.id : undefined}
      title={`${entry.name} — ${kindLabel(entry)}`}
      className={[
        "absolute inset-x-1 flex cursor-default items-center gap-2 rounded-md px-2 text-[13px]",
        isSelected
          ? "bg-selection text-cream"
          : props.highlighted
            ? props.dimHighlight
              ? "bg-charcoal-hover text-cream"
              : "bg-selection text-cream"
            : "text-cream hover:bg-charcoal-card",
        entry.hidden || source.dragging ? "opacity-60" : "",
      ].join(" ")}
      style={{ top: props.top, height: ROW_HEIGHT - 2 }}
      onPointerDown={source.onPointerDown}
      onClick={props.onClick}
      onDoubleClick={props.onDoubleClick}
      onContextMenu={(event) => {
        if (!props.onContextMenu) return;
        event.preventDefault();
        event.stopPropagation();
        props.onContextMenu(event);
      }}
    >
      <FileIcon entry={entry} size={16} variant="table" />
      {props.inlineEdit ? (
        <InlineNameEditor
          edit={props.inlineEdit}
          variant="table"
          onChange={useExplorerStore.getState().setInlineEditValue}
          onCommit={() => void useExplorerStore.getState().commitInlineEdit()}
          onCancel={useExplorerStore.getState().cancelInlineEdit}
        />
      ) : (
        <span className="min-w-0 flex-1 truncate">{entry.name}</span>
      )}
      {folder ? (
        <ChevronRight size={14} className="flex-none opacity-60" />
      ) : null}
    </div>
  );
}

/**
 * Folders above `path`, from Home when `path` is inside it, else from the
 * root. Virtual locations (kura://…) have no columns above them.
 */
export function ancestorPaths(path: string, homePath: string): string[] {
  if (!path.startsWith("/")) return [];
  const home = homePath.replace(/\/+$/, "");
  const root =
    home && (path === home || path.startsWith(`${home}/`)) ? home : "";
  const parts = path.slice(root.length).split("/").filter(Boolean);
  const ancestors: string[] = [root || "/"];
  let current = root;
  for (const part of parts.slice(0, -1)) {
    current = `${current}/${part}`;
    ancestors.push(current);
  }
  return path === (root || "/") ? [] : ancestors;
}

/** Listings for the folders above the current one, cached by path. */
function useAncestorListings(
  paths: string[],
  showHidden: boolean,
  current: DirectoryListing,
): Record<string, FileEntry[]> {
  const [listings, setListings] = useState<Record<string, FileEntry[]>>({});
  const key = paths.join("\n");
  useEffect(() => {
    let cancelled = false;
    for (const path of paths) {
      explorerListDirectory({ path, showHidden })
        .then((listing) => {
          if (!cancelled)
            setListings((previous) => ({
              ...previous,
              [path]: listing.entries,
            }));
        })
        .catch(() => {
          if (!cancelled)
            setListings((previous) => ({ ...previous, [path]: [] }));
        });
    }
    return () => {
      cancelled = true;
    };
    // `current` refreshes the parents when the folder is reloaded, which
    // is when a rename or move could have changed them.
  }, [key, showHidden, current]);
  return listings;
}

// Selecting the folder a user came back from, when going left.
export function selectChildOnReturn(paneId: string, childPath: string): void {
  explorerRuntime.pendingSelections.set(paneId, {
    path: childPath,
    expiresAtMs: Date.now() + 10_000,
  });
}
