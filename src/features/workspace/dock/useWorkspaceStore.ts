import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";
import {
  collapseEmptyDockLeaves,
  createDockId,
  createDockLeaf,
  dockLeaves,
  dockTreeViews,
  findDockLeaf,
  insertDockSplit,
  mapDockLeaf,
  mapDockTreeViews,
  moveDockPane,
  normalizePaneLayout,
  swapDockLeaves,
  updateDockSplitRatio,
} from "./dockTree";
import {
  activateTab,
  activeLayoutView,
  allLayoutViews,
  appendTab,
  commitActiveTab,
  emptyTab,
  isEmptyTab,
  singleViewTab,
} from "./layoutTabs";
import {
  maxWorkspacePanels,
  surfaceTitles,
  type DockDropZone,
  type DockSplitDirection,
  type FilesViewState,
  type WorkspaceDockNode,
  type WorkspaceLayout,
  type WorkspaceSurfaceId,
  type WorkspaceTab,
  type WorkspaceView,
  type Workspace,
} from "./model";

export interface OpenSurfaceRequest {
  surfaceId: WorkspaceSurfaceId;
  title?: string;
  state?: FilesViewState;
  /** Open another instance even when one is already open. */
  forceNew?: boolean;
}

export interface WorkspaceStore {
  /** The active workspace's tabs; every tab and pane action works on these. */
  layout: WorkspaceLayout;
  workspaces: Workspace[];
  activeWorkspaceId: string;
  closedTabs: WorkspaceTab[];
  createWorkspace: (title?: string) => Workspace;
  switchWorkspace: (id: string) => boolean;
  renameWorkspace: (id: string, title: string) => void;
  /** The last workspace cannot be deleted. */
  deleteWorkspace: (id: string) => boolean;
  newTab: () => WorkspaceView;
  openSurface: (request: OpenSurfaceRequest) => WorkspaceView;
  /** Resume the most recent view of a surface, or open one. */
  openDestination: (surfaceId: WorkspaceSurfaceId) => WorkspaceView;
  commitPlaceholder: (viewId: string) => void;
  updateViewState: (
    viewId: string,
    state: FilesViewState,
    title?: string,
  ) => void;
  focusPane: (paneId: string) => boolean;
  focusView: (viewId: string) => boolean;
  selectTab: (id: string) => WorkspaceView | null;
  selectTabAt: (index: number | "last") => WorkspaceView | null;
  cycleTab: (direction: 1 | -1) => WorkspaceView | null;
  renameTab: (id: string, title: string) => void;
  moveTab: (id: string, targetId: string, after: boolean) => void;
  closeTab: (id: string) => boolean;
  closeView: (viewId: string) => boolean;
  closePane: (paneId: string) => void;
  reopenClosedTab: () => WorkspaceView | null;
  splitPane: (
    paneId: string,
    direction: DockSplitDirection,
    viewId?: string,
  ) => string | null;
  dockView: (viewId: string, paneId: string, zone: DockDropZone) => boolean;
  movePane: (
    paneId: string,
    direction: DockSplitDirection,
    targetPaneId?: string,
  ) => boolean;
  swapPanes: (firstPaneId: string, secondPaneId: string) => boolean;
  updateSplitRatio: (splitId: string, ratio: number) => void;
}

export function createWorkspaceView(
  surfaceId: WorkspaceSurfaceId,
  options: {
    title?: string;
    state?: FilesViewState;
    placeholder?: boolean;
  } = {},
): WorkspaceView {
  const now = Date.now();
  return {
    id: createDockId("tab"),
    surfaceId,
    title: options.title ?? surfaceTitles[surfaceId],
    state: options.state ?? {},
    createdAt: now,
    lastFocusedAt: now,
    ...(options.placeholder ? { placeholder: true } : {}),
  };
}

/** A new tab or split starts on Files and gives way to the next destination. */
function blankView() {
  return createWorkspaceView("files", { placeholder: true });
}

function initialLayout(): WorkspaceLayout {
  const tab = singleViewTab(createWorkspaceView("files"));
  return activateTab({ ...tab, tabs: [tab], activeTabId: tab.id }, tab.id);
}

function createWorkspace(title: string, layout = initialLayout()): Workspace {
  return {
    id: createDockId("workspace"),
    title,
    layout,
    createdAt: Date.now(),
  };
}

/** "Workspace N" with the first N no other workspace is using. */
function nextWorkspaceTitle(workspaces: Workspace[]): string {
  const taken = new Set(workspaces.map((workspace) => workspace.title));
  let index = workspaces.length + 1;
  for (let n = 1; n <= workspaces.length + 1; n++)
    if (!taken.has(`Workspace ${n}`)) {
      index = n;
      break;
    }
  return `Workspace ${index}`;
}

function initialWorkspaces() {
  const workspace = createWorkspace("Workspace 1");
  return {
    layout: workspace.layout,
    workspaces: [workspace],
    activeWorkspaceId: workspace.id,
  };
}

function byRecency(a: WorkspaceView, b: WorkspaceView) {
  return b.lastFocusedAt - a.lastFocusedAt || b.createdAt - a.createdAt;
}

function mapLayoutViews(
  layout: WorkspaceLayout,
  update: (view: WorkspaceView) => WorkspaceView,
): WorkspaceLayout {
  const tabs = layout.tabs.map((tab) => {
    const root = mapDockTreeViews(tab.root, update);
    return root === tab.root ? tab : { ...tab, root };
  });
  return tabs.every((tab, index) => tab === layout.tabs[index])
    ? layout
    : activateTab({ ...layout, tabs }, layout.activeTabId);
}

function ownerTab(layout: WorkspaceLayout, viewId: string) {
  return layout.tabs.find((tab) =>
    dockTreeViews(tab.root).some((view) => view.id === viewId),
  );
}

function removeView(
  node: WorkspaceDockNode,
  viewId: string,
): WorkspaceDockNode {
  if (node.type === "leaf") {
    if (!node.views.some((view) => view.id === viewId)) return node;
    const views = node.views.filter((view) => view.id !== viewId);
    return { ...node, views, activeViewId: views[0]?.id ?? null };
  }
  const first = removeView(node.first, viewId);
  const second = removeView(node.second, viewId);
  return first === node.first && second === node.second
    ? node
    : { ...node, first, second };
}

/**
 * A saved layout with views of retired surfaces dropped, or null when it is
 * unreadable or nothing is left of it.
 */
function restoredLayout(value: unknown): WorkspaceLayout | null {
  const layout = value as WorkspaceLayout | undefined;
  if (!layout || !Array.isArray(layout.tabs)) return null;
  const tabs = layout.tabs.flatMap((tab): WorkspaceTab[] => {
    if (!tab?.root) return [];
    const retired = dockTreeViews(tab.root).filter(
      (view) => !view || !(view.surfaceId in surfaceTitles),
    );
    const root = retired.reduce<WorkspaceDockNode | null>(
      (node, view) =>
        node && collapseEmptyDockLeaves(removeView(node, view.id)),
      tab.root,
    );
    if (!root) return [];
    const focusedPaneId = findDockLeaf(root, tab.focusedPaneId)
      ? tab.focusedPaneId
      : dockLeaves(root)[0].id;
    return [{ ...tab, root, focusedPaneId }];
  });
  if (!tabs.length) return null;
  return activateTab(
    { ...layout, tabs },
    tabs.some((tab) => tab.id === layout.activeTabId)
      ? layout.activeTabId
      : tabs[0].id,
  );
}

interface PersistedWorkspaces {
  layout?: unknown;
  workspaces?: {
    id?: unknown;
    title?: unknown;
    layout?: unknown;
    createdAt?: unknown;
  }[];
  activeWorkspaceId?: unknown;
}

function restoredWorkspaces(
  persisted: PersistedWorkspaces | undefined,
): Pick<WorkspaceStore, "layout" | "workspaces" | "activeWorkspaceId"> | null {
  const layout = restoredLayout(persisted?.layout);
  const saved = Array.isArray(persisted?.workspaces)
    ? persisted.workspaces
    : [];
  const workspaces = saved.flatMap((entry): Workspace[] => {
    if (typeof entry?.id !== "string" || typeof entry.title !== "string")
      return [];
    return [
      {
        id: entry.id,
        title: entry.title,
        layout: restoredLayout(entry.layout) ?? initialLayout(),
        createdAt: typeof entry.createdAt === "number" ? entry.createdAt : 0,
      },
    ];
  });
  const active = workspaces.find(
    (workspace) => workspace.id === persisted?.activeWorkspaceId,
  );
  if (active)
    return {
      layout: layout ?? active.layout,
      workspaces,
      activeWorkspaceId: active.id,
    };
  // Layouts saved before workspaces existed become the first workspace.
  if (!layout) return null;
  const first = createWorkspace(nextWorkspaceTitle(workspaces), layout);
  return {
    layout,
    workspaces: [first, ...workspaces],
    activeWorkspaceId: first.id,
  };
}

export const useWorkspaceStore = create<WorkspaceStore>()(
  persist(
    (set, get) => {
      const setLayout = (layout: WorkspaceLayout) => set({ layout });
      /** Every workspace, with the active one holding the current layout. */
      const savedWorkspaces = () => {
        const { workspaces, activeWorkspaceId, layout } = get();
        return workspaces.map((workspace) =>
          workspace.id === activeWorkspaceId
            ? { ...workspace, layout }
            : workspace,
        );
      };
      const enterWorkspace = (workspaces: Workspace[], target: Workspace) =>
        set({
          workspaces,
          activeWorkspaceId: target.id,
          layout: target.layout,
        });
      return {
        ...initialWorkspaces(),
        closedTabs: [],
        createWorkspace: (title) => {
          const workspaces = savedWorkspaces();
          const workspace = createWorkspace(
            title?.trim() || nextWorkspaceTitle(workspaces),
          );
          enterWorkspace([...workspaces, workspace], workspace);
          return workspace;
        },
        switchWorkspace: (id) => {
          if (id === get().activeWorkspaceId) return true;
          const workspaces = savedWorkspaces();
          const target = workspaces.find((workspace) => workspace.id === id);
          if (!target) return false;
          enterWorkspace(workspaces, target);
          return true;
        },
        renameWorkspace: (id, title) => {
          const trimmed = title.trim();
          if (!trimmed) return;
          set({
            workspaces: get().workspaces.map((workspace) =>
              workspace.id === id
                ? { ...workspace, title: trimmed }
                : workspace,
            ),
          });
        },
        deleteWorkspace: (id) => {
          const workspaces = savedWorkspaces();
          const index = workspaces.findIndex(
            (workspace) => workspace.id === id,
          );
          if (index < 0 || workspaces.length < 2) return false;
          const remaining = workspaces.filter(
            (workspace) => workspace.id !== id,
          );
          if (id !== get().activeWorkspaceId) {
            set({ workspaces: remaining });
            return true;
          }
          enterWorkspace(
            remaining,
            remaining[Math.min(index, remaining.length - 1)],
          );
          return true;
        },
        newTab: () => {
          const view = blankView();
          setLayout(appendTab(get().layout, singleViewTab(view)));
          return view;
        },
        openSurface: (request) => {
          const { layout } = get();
          const active = activeLayoutView(layout);
          const view = createWorkspaceView(request.surfaceId, {
            title: request.title,
            state: request.state,
          });
          // An unused new tab or split takes the destination in place.
          if (active?.placeholder) {
            const pane = dockLeaves(layout.root).find((leaf) =>
              leaf.views.some((item) => item.id === active.id),
            )!;
            setLayout(
              commitActiveTab(
                layout,
                mapDockLeaf(layout.root, pane.id, (leaf) => ({
                  ...leaf,
                  views: [view],
                  activeViewId: view.id,
                })),
                pane.id,
              ),
            );
            return view;
          }
          setLayout(appendTab(layout, singleViewTab(view)));
          return view;
        },
        openDestination: (surfaceId) => {
          const { layout } = get();
          if (!activeLayoutView(layout)?.placeholder) {
            const existing = allLayoutViews(layout)
              .filter(
                (view) => !view.placeholder && view.surfaceId === surfaceId,
              )
              .sort(byRecency)[0];
            if (existing) {
              get().focusView(existing.id);
              return existing;
            }
          }
          return get().openSurface({ surfaceId });
        },
        commitPlaceholder: (viewId) => {
          const { layout } = get();
          if (
            !allLayoutViews(layout).some(
              (v) => v.id === viewId && v.placeholder,
            )
          )
            return;
          setLayout(
            mapLayoutViews(layout, (view) =>
              view.id === viewId ? { ...view, placeholder: false } : view,
            ),
          );
        },
        updateViewState: (viewId, state, title) => {
          setLayout(
            mapLayoutViews(get().layout, (view) =>
              view.id === viewId
                ? { ...view, state, title: title?.trim() || view.title }
                : view,
            ),
          );
        },
        focusPane: (paneId) => {
          const { layout } = get();
          const pane = findDockLeaf(layout.root, paneId);
          if (!pane) return false;
          if (pane.views[0]) return get().focusView(pane.views[0].id);
          if (layout.focusedPaneId !== paneId)
            setLayout(commitActiveTab(layout, layout.root, paneId));
          return true;
        },
        focusView: (viewId) => {
          let { layout } = get();
          const owner = ownerTab(layout, viewId);
          if (!owner) return false;
          if (owner.id !== layout.activeTabId)
            layout = activateTab(layout, owner.id);
          const pane = dockLeaves(layout.root).find((leaf) =>
            leaf.views.some((view) => view.id === viewId),
          )!;
          if (
            layout === get().layout &&
            layout.focusedPaneId === pane.id &&
            pane.activeViewId === viewId
          )
            return true;
          const now = Date.now();
          setLayout(
            commitActiveTab(
              layout,
              mapDockLeaf(layout.root, pane.id, (leaf) => ({
                ...leaf,
                activeViewId: viewId,
                views: leaf.views.map((view) =>
                  view.id === viewId ? { ...view, lastFocusedAt: now } : view,
                ),
              })),
              pane.id,
            ),
          );
          return true;
        },
        selectTab: (id) => {
          const tab = get().layout.tabs.find(
            (candidate) => candidate.id === id,
          );
          if (!tab) return null;
          const view = activeLayoutView(tab);
          if (view) get().focusView(view.id);
          else setLayout(activateTab(get().layout, id));
          return view;
        },
        selectTabAt: (index) => {
          const tabs = get().layout.tabs.filter((tab) => !isEmptyTab(tab));
          const tab = index === "last" ? tabs[tabs.length - 1] : tabs[index];
          return tab ? get().selectTab(tab.id) : null;
        },
        cycleTab: (direction) => {
          const { layout } = get();
          const tabs = layout.tabs.filter((tab) => !isEmptyTab(tab));
          if (tabs.length < 2) return null;
          const index = Math.max(
            0,
            tabs.findIndex((tab) => tab.id === layout.activeTabId),
          );
          return get().selectTab(
            tabs[(index + direction + tabs.length) % tabs.length].id,
          );
        },
        renameTab: (id, title) => {
          const { layout } = get();
          setLayout(
            activateTab(
              {
                ...layout,
                tabs: layout.tabs.map((tab) =>
                  tab.id === id
                    ? { ...tab, title: title.trim() || undefined }
                    : tab,
                ),
              },
              layout.activeTabId,
            ),
          );
        },
        moveTab: (id, targetId, after) => {
          const { layout } = get();
          const moving = layout.tabs.find((tab) => tab.id === id);
          if (!moving || id === targetId) return;
          const rest = layout.tabs.filter((tab) => tab.id !== id);
          const index = rest.findIndex((tab) => tab.id === targetId);
          if (index < 0) return;
          rest.splice(after ? index + 1 : index, 0, moving);
          setLayout(activateTab({ ...layout, tabs: rest }, layout.activeTabId));
        },
        closeTab: (id) => {
          const { layout, closedTabs } = get();
          const closing = layout.tabs.find((tab) => tab.id === id);
          if (!closing) return false;
          const index = layout.tabs.indexOf(closing);
          let remaining = layout.tabs.filter((tab) => tab.id !== id);
          if (!remaining.length) remaining = [emptyTab()];
          const next =
            remaining.find((tab) => tab.id === layout.activeTabId) ??
            remaining[Math.min(index, remaining.length - 1)];
          set({
            layout: activateTab({ ...layout, tabs: remaining }, next.id),
            closedTabs: isEmptyTab(closing)
              ? closedTabs
              : [closing, ...closedTabs].slice(0, 20),
          });
          return true;
        },
        closeView: (viewId) => {
          const { layout } = get();
          const owner = ownerTab(layout, viewId);
          if (!owner) return false;
          if (dockTreeViews(owner.root).length === 1)
            return get().closeTab(owner.id);
          const root = collapseEmptyDockLeaves(removeView(owner.root, viewId));
          if (!root) return false;
          const focusedPaneId = findDockLeaf(root, owner.focusedPaneId)
            ? owner.focusedPaneId
            : dockLeaves(root)[0].id;
          setLayout(
            activateTab(
              {
                ...layout,
                tabs: layout.tabs.map((tab) =>
                  tab.id === owner.id ? { ...tab, root, focusedPaneId } : tab,
                ),
              },
              layout.activeTabId,
            ),
          );
          return true;
        },
        closePane: (paneId) => {
          const { layout } = get();
          const pane = findDockLeaf(layout.root, paneId);
          if (dockLeaves(layout.root).length > 1 && pane?.views[0])
            get().closeView(pane.views[0].id);
        },
        reopenClosedTab: () => {
          const [closed, ...rest] = get().closedTabs;
          if (!closed) return null;
          set({ layout: appendTab(get().layout, closed), closedTabs: rest });
          return activeLayoutView(closed);
        },
        splitPane: (paneId, direction, viewId) => {
          if (viewId) {
            if (!get().dockView(viewId, paneId, direction)) return null;
            return (
              dockLeaves(get().layout.root).find((pane) =>
                pane.views.some((view) => view.id === viewId),
              )?.id ?? null
            );
          }
          const { layout } = get();
          if (dockLeaves(layout.root).length >= maxWorkspacePanels) return null;
          if (!findDockLeaf(layout.root, paneId)) return null;
          const leaf = createDockLeaf([blankView()]);
          setLayout(
            commitActiveTab(
              layout,
              normalizePaneLayout(
                insertDockSplit(layout.root, paneId, leaf, direction),
              ),
              leaf.id,
            ),
          );
          return leaf.id;
        },
        dockView: (viewId, paneId, zone) => {
          const { layout } = get();
          const source = ownerTab(layout, viewId);
          const targetTab = layout.tabs.find((tab) =>
            dockLeaves(tab.root).some((pane) => pane.id === paneId),
          );
          const target = targetTab && findDockLeaf(targetTab.root, paneId);
          const sourcePane =
            source &&
            dockLeaves(source.root).find((pane) =>
              pane.views.some((view) => view.id === viewId),
            );
          const moving = sourcePane?.views.find((view) => view.id === viewId);
          if (!source || !targetTab || !target || !sourcePane || !moving)
            return false;
          if (sourcePane.id === paneId) return false;
          if (
            zone !== "center" &&
            dockLeaves(targetTab.root).length >= maxWorkspacePanels
          )
            return false;
          const same = source.id === targetTab.id;
          let sourceRoot = source.root;
          let targetRoot = targetTab.root;
          if (zone === "center") {
            // A pane shows one view; dropping in its center exchanges them.
            sourceRoot = mapDockLeaf(sourceRoot, sourcePane.id, (pane) => ({
              ...pane,
              views: target.views,
              activeViewId: target.activeViewId,
            }));
            targetRoot = mapDockLeaf(
              same ? sourceRoot : targetRoot,
              paneId,
              (pane) => ({
                ...pane,
                views: [moving],
                activeViewId: moving.id,
              }),
            );
          } else {
            sourceRoot = removeView(sourceRoot, moving.id);
            targetRoot = insertDockSplit(
              same ? sourceRoot : targetRoot,
              paneId,
              createDockLeaf([moving]),
              zone,
            );
          }
          const destinationRoot = normalizePaneLayout(
            collapseEmptyDockLeaves(targetRoot)!,
          );
          const sourceRemaining = collapseEmptyDockLeaves(sourceRoot);
          const destinationPane = dockLeaves(destinationRoot).find((pane) =>
            pane.views.some((view) => view.id === moving.id),
          )!;
          const tabs = layout.tabs.flatMap((tab) => {
            if (tab.id === targetTab.id)
              return [
                {
                  ...tab,
                  root: destinationRoot,
                  focusedPaneId: destinationPane.id,
                },
              ];
            if (tab.id !== source.id) return [tab];
            return sourceRemaining
              ? [
                  {
                    ...tab,
                    root: sourceRemaining,
                    focusedPaneId: dockLeaves(sourceRemaining)[0].id,
                  },
                ]
              : [];
          });
          setLayout(activateTab({ ...layout, tabs }, targetTab.id));
          return true;
        },
        movePane: (paneId, direction, targetPaneId) => {
          const { layout } = get();
          const root = moveDockPane(
            layout.root,
            paneId,
            direction,
            targetPaneId,
          );
          if (root === layout.root) return false;
          setLayout(commitActiveTab(layout, root, paneId));
          return true;
        },
        swapPanes: (firstPaneId, secondPaneId) => {
          const { layout } = get();
          const root = swapDockLeaves(
            normalizePaneLayout(layout.root),
            firstPaneId,
            secondPaneId,
          );
          if (root === layout.root) return false;
          setLayout(commitActiveTab(layout, root, secondPaneId));
          return true;
        },
        updateSplitRatio: (splitId, ratio) => {
          const { layout } = get();
          const root = updateDockSplitRatio(layout.root, splitId, ratio);
          if (root !== layout.root) setLayout(commitActiveTab(layout, root));
        },
      };
    },
    {
      name: "kura:workspace-layout",
      version: 1,
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({
        layout: state.layout,
        // The active workspace's layout is already saved as `layout`.
        workspaces: state.workspaces.map((workspace) =>
          workspace.id === state.activeWorkspaceId
            ? { ...workspace, layout: undefined }
            : workspace,
        ),
        activeWorkspaceId: state.activeWorkspaceId,
      }),
      merge: (persisted, current) => ({
        ...current,
        ...restoredWorkspaces(persisted as PersistedWorkspaces | undefined),
      }),
    },
  ),
);
