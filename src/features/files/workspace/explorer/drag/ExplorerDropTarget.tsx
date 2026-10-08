import { useMultiPanelStoreContext } from "@/features/workspace/MultiPanelWorkspace";
import {
  useMultiPanelStore,
  type MultiPanelStoreHook,
} from "@/features/workspace/useMultiPanelStore";
import { explorerQueueDeleteItems } from "../../native";
import { useMemo, type ReactNode } from "react";
import {
  transferDropAcceptance,
  type ExplorerDragModifiers,
  type ExplorerDragPayload,
  type ExplorerDropZoneSpec,
  groupItemsByOperation,
  storageIdForPath,
} from "@/features/file-ui";
import { useExplorerStore } from "../store";
import { Droppable } from "./ExplorerDragContext";

export function ExplorerDropTarget(props: {
  id: string;
  path: string;
  kind?: TargetKind;
  paneId?: string;
  remoteName?: string | null;
  springLoad?: boolean;
  onSpringLoad?: () => void;
  children: ReactNode;
}) {
  const multiPanelStore = useMultiPanelStoreContext();
  const kind = props.kind ?? dropKindForPath(props.path);
  const zone = useMemo<ExplorerDropZoneSpec>(
    () =>
      createExplorerDropTargetSpec({
        id: props.id,
        path: props.path,
        kind,
        paneId: props.paneId,
        remoteName: props.remoteName,
        onSpringLoad: props.onSpringLoad,
        springLoad: props.springLoad,
        multiPanelStore: multiPanelStore ?? undefined,
      }),
    [
      kind,
      props.id,
      props.onSpringLoad,
      props.paneId,
      props.path,
      props.remoteName,
      props.springLoad,
      multiPanelStore,
    ],
  );
  return (
    <Droppable className="contents" zone={zone}>
      {props.children}
    </Droppable>
  );
}

export function createExplorerDropTargetSpec(options: {
  id: string;
  path: string;
  kind?: TargetKind;
  paneId?: string;
  remoteName?: string | null;
  springLoad?: boolean;
  onSpringLoad?: () => void;
  multiPanelStore?: MultiPanelStoreHook;
}): ExplorerDropZoneSpec {
  const kind = options.kind ?? dropKindForPath(options.path);
  return {
    id: options.id,
    priority: 10,
    accepts: (payload) => acceptanceForTarget(kind, payload, options.path),
    onDrop: (payload, modifiers) =>
      dropOnTarget(
        kind,
        payload,
        options.path,
        options.remoteName,
        options.paneId,
        modifiers,
        options.multiPanelStore,
      ),
    onSpringLoad: options.onSpringLoad,
    springLoad: options.springLoad,
  };
}

export function dropKindForPath(path: string): TargetKind {
  if (path === "kura://trash") return "trash";
  if (path.startsWith("kura://")) return "invalid";
  return "directory";
}

function acceptanceForTarget(
  kind: TargetKind,
  payload: ExplorerDragPayload,
  destination: string,
) {
  if (kind === "invalid")
    return {
      valid: false,
      label: "Not a drop destination",
      action: "invalid" as const,
    };
  if (kind === "trash")
    return { valid: true, label: "Move to Trash", action: "trash" as const };
  return transferDropAcceptance(payload, destination);
}

async function dropOnTarget(
  kind: TargetKind,
  payload: ExplorerDragPayload,
  destination: string,
  remoteName: string | null | undefined,
  paneId: string | undefined,
  modifiers: ExplorerDragModifiers,
  multiPanelStore?: MultiPanelStoreHook,
) {
  const explorer = useExplorerStore.getState();
  const targetPaneId =
    paneId ?? (multiPanelStore ?? useMultiPanelStore).getState().activePaneId;
  if (kind === "trash") {
    await explorerQueueDeleteItems({
      paths: payload.items.map((item) => item.path),
      permanent: false,
    });
    explorer.pushNotification(
      `Queued ${payload.items.length} item${payload.items.length === 1 ? "" : "s"} for Trash`,
      "success",
    );
    refreshAllPanes();
    return;
  }
  if (kind !== "directory" || !targetPaneId) return;
  if (payload.origin === "external") {
    await explorer.dropExternalPaths(
      targetPaneId,
      payload.items.map((item) => item.path),
      destination,
    );
    return;
  }
  const groups = groupItemsByOperation(
    payload.items,
    storageIdForPath(destination, remoteName),
    modifiers.copyRequested,
  );
  if (groups.move.length)
    await explorer.dropItems(targetPaneId, groups.move, destination, "move");
  if (groups.copy.length)
    await explorer.dropItems(targetPaneId, groups.copy, destination, "copy");
}

function refreshAllPanes() {
  const explorer = useExplorerStore.getState();
  Object.keys(explorer.panes).forEach(
    (paneId) => void explorer.refreshPane(paneId),
  );
}

export type TargetKind = "directory" | "trash" | "invalid";
