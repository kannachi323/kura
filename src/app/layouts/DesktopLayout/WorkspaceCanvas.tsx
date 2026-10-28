import { registerShortcutHandler } from "@/features/shortcuts";
import {
  canFitDockSplit,
  dockLeaves,
  findDockLeaf,
  maxWorkspacePanels,
  tabLabel,
  useWorkspaceStore,
  type DockSplitDirection,
} from "@/features/workspace";
import { useEffect } from "react";
import {
  minimumForWorkspaceViews,
  WorkspaceDockTree,
} from "./WorkspaceDockTree";
import { WorkspaceTabStrip } from "./WorkspaceTabStrip";

/** Window tabs above their split-pane trees; every open tab stays mounted. */
export function WorkspaceCanvas(props: {
  titlebarInsets?: { left: number; right: number; animate?: boolean };
}) {
  const layout = useWorkspaceStore((state) => state.layout);
  const store = useWorkspaceStore.getState;

  useEffect(() => {
    const paneBounds = () =>
      dockLeaves(store().layout.root).flatMap((pane) => {
        const element = document.querySelector<HTMLElement>(
          `[data-workspace-pane="${CSS.escape(pane.id)}"]`,
        );
        if (!element) return [];
        const rect = element.getBoundingClientRect();
        return [{ id: pane.id, rect }];
      });
    const paneInDirection = (direction: DockSplitDirection) => {
      const bounds = paneBounds();
      const current = bounds.find(
        (pane) => pane.id === store().layout.focusedPaneId,
      )?.rect;
      if (!current) return null;
      const cx = current.left + current.width / 2,
        cy = current.top + current.height / 2;
      return (
        bounds
          .filter(({ rect }) =>
            direction === "left"
              ? rect.right <= current.left + 1
              : direction === "right"
                ? rect.left >= current.right - 1
                : direction === "up"
                  ? rect.bottom <= current.top + 1
                  : rect.top >= current.bottom - 1,
          )
          .sort(
            (a, b) =>
              Math.hypot(
                a.rect.left + a.rect.width / 2 - cx,
                a.rect.top + a.rect.height / 2 - cy,
              ) -
              Math.hypot(
                b.rect.left + b.rect.width / 2 - cx,
                b.rect.top + b.rect.height / 2 - cy,
              ),
          )[0]?.id ?? null
      );
    };
    const canSplit = (direction: DockSplitDirection) => {
      const { layout } = store();
      if (dockLeaves(layout.root).length >= maxWorkspacePanels) return false;
      const pane = findDockLeaf(layout.root, layout.focusedPaneId);
      const rect = paneBounds().find((entry) => entry.id === pane?.id)?.rect;
      return Boolean(
        pane &&
        rect &&
        canFitDockSplit(rect, direction, minimumForWorkspaceViews(pane.views), {
          width: 280,
          height: 180,
        }),
      );
    };
    const unregister = [
      registerShortcutHandler("explorer.new_tab", () => void store().newTab()),
      registerShortcutHandler("explorer.close_tab", () => {
        const { layout } = store();
        const pane = findDockLeaf(layout.root, layout.focusedPaneId);
        // In a split, close the focused pane; otherwise the whole tab.
        if (dockLeaves(layout.root).length > 1 && pane?.views[0])
          store().closeView(pane.views[0].id);
        else store().closeTab(layout.activeTabId);
      }),
      registerShortcutHandler(
        "explorer.restore_tab",
        () => void store().reopenClosedTab(),
        () => store().closedTabs.length > 0,
      ),
      registerShortcutHandler(
        "explorer.next_workspace",
        () => void store().cycleTab(1),
      ),
      registerShortcutHandler(
        "workspace.previous_tab",
        () => void store().cycleTab(-1),
      ),
      ...Array.from({ length: 9 }, (_, index) =>
        registerShortcutHandler(
          `explorer.tab_${index + 1}`,
          () => void store().selectTabAt(index === 8 ? "last" : index),
        ),
      ),
      registerShortcutHandler(
        "workspace.split_right",
        () => void store().splitPane(store().layout.focusedPaneId, "right"),
        () => canSplit("right"),
      ),
      registerShortcutHandler(
        "workspace.split_down",
        () => void store().splitPane(store().layout.focusedPaneId, "down"),
        () => canSplit("down"),
      ),
      registerShortcutHandler(
        "workspace.close_pane",
        () => store().closePane(store().layout.focusedPaneId),
        () => dockLeaves(store().layout.root).length > 1,
      ),
      ...(["left", "right", "up", "down"] as const).map((direction) =>
        registerShortcutHandler(
          `workspace.focus_pane_${direction}`,
          () => {
            const paneId = paneInDirection(direction);
            if (paneId) store().focusPane(paneId);
          },
          () => Boolean(paneInDirection(direction)),
        ),
      ),
    ];
    return () => unregister.forEach((remove) => remove());
  }, [store]);

  const activeLabel = layout.tabs.find((tab) => tab.id === layout.activeTabId);
  useEffect(() => {
    document.title = activeLabel ? `${tabLabel(activeLabel)} - Kura` : "Kura";
  }, [activeLabel]);

  return (
    <div
      className="flex h-full min-h-0 min-w-0 flex-col overflow-hidden bg-charcoal-border"
      data-workspace-panes={dockLeaves(layout.root).length}
    >
      <WorkspaceTabStrip
        titlebarInsets={props.titlebarInsets}
        onOpen={(view) => store().focusView(view.id)}
        onNewTab={() => store().newTab()}
        onCloseTab={(id) => store().closeTab(id)}
        onSplitPane={(paneId, direction) =>
          store().splitPane(paneId, direction)
        }
        onClosePane={(paneId) => store().closePane(paneId)}
      />
      <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
        {layout.tabs.map((tab) => {
          const active = tab.id === layout.activeTabId;
          return (
            <div
              key={tab.id}
              className={
                active ? "min-h-0 min-w-0 flex-1 overflow-hidden" : "hidden"
              }
              role="tabpanel"
              aria-label={tabLabel(tab)}
              aria-hidden={!active}
              inert={!active}
            >
              <WorkspaceDockTree
                node={tab.root}
                workspaceActive={active}
                focusedPaneId={tab.focusedPaneId}
                onOpen={(view) => store().focusView(view.id)}
                onClosePane={(paneId) => store().closePane(paneId)}
                onResizeSplit={(splitId, ratio) =>
                  store().updateSplitRatio(splitId, ratio)
                }
              />
            </div>
          );
        })}
      </div>
    </div>
  );
}
