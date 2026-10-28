/** The pages a window tab or split pane can show. */
export type WorkspaceSurfaceId = "files" | "transfers";

export type DockSplitDirection = "left" | "right" | "up" | "down";
export type DockDropZone = "center" | DockSplitDirection;

/** What a Files view remembers so it reopens on the same folder. */
export interface FilesViewState {
  path?: string;
}

export interface WorkspaceView {
  /** An unused new tab or split; the next destination picked replaces it. */
  placeholder?: boolean;
  id: string;
  surfaceId: WorkspaceSurfaceId;
  title: string;
  state: FilesViewState;
  createdAt: number;
  lastFocusedAt: number;
}

export interface WorkspacePane {
  type: "leaf";
  id: string;
  views: WorkspaceView[];
  activeViewId: string | null;
}

export interface WorkspaceSplit {
  type: "split";
  id: string;
  direction: "horizontal" | "vertical";
  ratio: number;
  first: WorkspaceDockNode;
  second: WorkspaceDockNode;
}

export type WorkspaceDockNode = WorkspacePane | WorkspaceSplit;

/** A window tab owns a split tree; each leaf holds one view. */
export interface WorkspaceTab {
  id: string;
  title?: string;
  root: WorkspaceDockNode;
  focusedPaneId: string;
}

export interface WorkspaceLayout {
  tabs: WorkspaceTab[];
  activeTabId: string;
  /** Projection of the active tab, kept for pane consumers. */
  root: WorkspaceDockNode;
  focusedPaneId: string;
}

/**
 * A named set of window tabs, like virtual desktops. Switching
 * workspaces swaps every tab and split in the window at once.
 */
export interface Workspace {
  id: string;
  title: string;
  /** Kept current for inactive workspaces; the active one lives in the store's `layout`. */
  layout: WorkspaceLayout;
  createdAt: number;
}

export const maxWorkspacePanels = 4;

export const surfaceTitles: Record<WorkspaceSurfaceId, string> = {
  files: "Files",
  transfers: "Transfers",
};

/** Smallest size each surface stays usable at, used before offering a split. */
export const surfaceMinimumSize: Record<
  WorkspaceSurfaceId,
  { width: number; height: number }
> = {
  files: { width: 360, height: 240 },
  transfers: { width: 360, height: 240 },
};
