import { createDockLeaf, dockLeaves, dockTreeViews } from "./dockTree";
import {
  surfaceTitles,
  type WorkspaceLayout,
  type WorkspaceTab,
  type WorkspaceView,
} from "./model";

export function allLayoutViews(layout: WorkspaceLayout): WorkspaceView[] {
  return layout.tabs.flatMap((tab) => dockTreeViews(tab.root));
}

export function activeLayoutView(
  layout: Pick<WorkspaceTab, "root" | "focusedPaneId">,
): WorkspaceView | null {
  const panes = dockLeaves(layout.root);
  const pane =
    panes.find((pane) => pane.id === layout.focusedPaneId) ?? panes[0];
  return (
    pane?.views.find((tab) => tab.id === pane.activeViewId) ??
    pane?.views[0] ??
    null
  );
}

/** Make a tab current, projecting its tree onto the layout. */
export function activateTab(
  layout: WorkspaceLayout,
  id: string,
): WorkspaceLayout {
  const active = layout.tabs.find((tab) => tab.id === id);
  return active
    ? {
        tabs: layout.tabs,
        activeTabId: id,
        root: active.root,
        focusedPaneId: active.focusedPaneId,
      }
    : layout;
}

/** Write the projected tree back into the active tab. */
export function commitActiveTab(
  layout: WorkspaceLayout,
  root = layout.root,
  focusedPaneId = layout.focusedPaneId,
): WorkspaceLayout {
  return activateTab(
    {
      ...layout,
      tabs: layout.tabs.map((tab) =>
        tab.id === layout.activeTabId ? { ...tab, root, focusedPaneId } : tab,
      ),
    },
    layout.activeTabId,
  );
}

/** A window with no tabs keeps one empty layout tab that is never shown. */
export function isEmptyTab(tab: WorkspaceTab): boolean {
  return dockTreeViews(tab.root).length === 0;
}

export function emptyTab(): WorkspaceTab {
  const pane = createDockLeaf();
  return { id: `layout:${pane.id}`, root: pane, focusedPaneId: pane.id };
}

export function appendTab(
  layout: WorkspaceLayout,
  tab: WorkspaceTab,
): WorkspaceLayout {
  const kept = layout.tabs.filter((t) => !isEmptyTab(t));
  return activateTab({ ...layout, tabs: [...kept, tab] }, tab.id);
}

export function singleViewTab(view: WorkspaceView): WorkspaceTab {
  const pane = createDockLeaf([view]);
  return { id: `layout:${view.id}`, root: pane, focusedPaneId: pane.id };
}

export function paneViewLabel(view: WorkspaceView | null | undefined): string {
  if (!view || view.placeholder) return "New Tab";
  return view.title?.trim() || surfaceTitles[view.surfaceId];
}

export function tabLabel(tab: WorkspaceTab): string {
  return tab.title || paneViewLabel(activeLayoutView(tab));
}
