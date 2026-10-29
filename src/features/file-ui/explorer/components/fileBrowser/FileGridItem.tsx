import type { FileEntry } from "@/native/ipc";
import { CloudStateIcon } from "../CloudStateIcon";
import type { CSSProperties, MouseEvent } from "react";
import { memo, useMemo } from "react";
import {
  useExplorerDragSource,
  useExplorerDropZone,
} from "../../drag/ExplorerDragHooks";
import { storageIdForPath } from "../../drag/operations";
import { isPackageEntry } from "../../utils/kindLabel";
import { EntryTagDots } from "./TagDots";
import type { FileBrowserProps } from "../../model/interfaces/components/FileBrowser";
import type {
  ExplorerDragModifiers,
  ExplorerDragPayload,
} from "../../model/interfaces/drag/types";
import type { FileBrowserDragItem } from "../../model/types/components/FileBrowserDrag";
import type { PassiveRenameDraft } from "../../model/types/components/FileBrowserInline";
import type { ExplorerInlineEditState } from "../../store/index";
import { transferDropAcceptance } from "../FileBrowserDrag";
import { InlineNameEditor, PassiveRenameDraftView } from "../FileBrowserInline";
import { fileBrowserStyles } from "../FileBrowserStyles";
import { GridThumbnail } from "./GridThumbnail";

export const FileGridItem = memo(function FileGridItem(props: {
  entry: FileEntry;
  selectionOnly: boolean;
  thumbnailsEnabled: boolean;
  iconSize: number;
  style: CSSProperties;
  selected: boolean;
  cut: boolean;
  inlineEdit: ExplorerInlineEditState | null;
  passiveRename: PassiveRenameDraft | null;
  onSelect: (entryId: string, event: MouseEvent) => void;
  onOpen: FileBrowserProps["onOpen"];
  onContextMenu: FileBrowserProps["onContextMenu"];
  dragItems: FileBrowserDragItem[];
  onDropItems: FileBrowserProps["onDropItems"];
  onInlineEditChange: FileBrowserProps["onInlineEditChange"];
  onInlineEditCommit: FileBrowserProps["onInlineEditCommit"];
  onInlineEditCancel: FileBrowserProps["onInlineEditCancel"];
}) {
  const { entry } = props;
  const source = useExplorerDragSource(
    props.selectionOnly || entry.isDeleted ? [] : props.dragItems,
  );
  const dropSpec = useMemo(
    () => ({
      id: `entry:${entry.id}`,
      priority: 20,
      accepts: (payload: ExplorerDragPayload) =>
        transferDropAcceptance(payload, entry.path, {
          // Dropping onto an app would move files inside its bundle.
          folder: entry.kind === "folder" && !isPackageEntry(entry),
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
      onSpringLoad:
        entry.kind === "folder" && !isPackageEntry(entry)
          ? () => props.onOpen(entry)
          : undefined,
      springLoad: entry.kind === "folder" && !isPackageEntry(entry),
    }),
    [entry, props],
  );
  const drop = useExplorerDropZone(dropSpec);

  return (
    <div
      ref={drop.ref}
      className={[
        fileBrowserStyles.gridItem,
        props.selected ? fileBrowserStyles.gridItemSelected : "",
        entry.isDeleted ? fileBrowserStyles.gridItemDeleted : "",
        source.dragging ? fileBrowserStyles.gridItemDragging : "",
        props.cut ? fileBrowserStyles.gridItemCut : "",
      ].join(" ")}
      style={props.style}
      aria-disabled={entry.isDeleted || undefined}
      aria-pressed={props.selected}
      data-entry-id={entry.id}
      role="button"
      tabIndex={entry.isDeleted ? -1 : 0}
      onPointerDown={source.onPointerDown}
      onClick={(event) => props.onSelect(entry.id, event)}
      onDoubleClick={(event) => {
        if (!entry.isDeleted)
          props.onOpen(entry, { newTab: event.metaKey || event.ctrlKey });
      }}
      onAuxClick={(event) => {
        if (event.button === 1 && entry.kind === "folder" && !entry.isDeleted)
          props.onOpen(entry, { newTab: true });
      }}
      onKeyDown={(event) => {
        // In the Explorer the pane handles Enter (rename on macOS); pickers open.
        if (event.key === "Enter" && props.selectionOnly && !entry.isDeleted)
          props.onOpen(entry);
      }}
      onContextMenu={
        props.selectionOnly
          ? (event) => event.preventDefault()
          : (event) => props.onContextMenu(event, entry)
      }
    >
      <GridThumbnail
        entry={entry}
        enabled={props.thumbnailsEnabled}
        iconSize={props.iconSize}
      />
      {props.inlineEdit ? (
        <InlineNameEditor
          edit={props.inlineEdit}
          variant="grid"
          onChange={props.onInlineEditChange}
          onCommit={props.onInlineEditCommit}
          onCancel={props.onInlineEditCancel}
        />
      ) : props.passiveRename ? (
        <PassiveRenameDraftView draft={props.passiveRename} />
      ) : (
        <span className="inline-flex min-w-0 max-w-full items-center justify-center gap-1">
          <span className={fileBrowserStyles.gridNameText}>{entry.name}</span>
          <EntryTagDots entry={entry} />
          <CloudStateIcon state={entry.location.cloudState} />
        </span>
      )}
    </div>
  );
});
