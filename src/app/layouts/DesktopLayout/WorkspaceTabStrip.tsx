import { Renameable } from "@/features/navigation-names/Renameable";
import {
  activeLayoutView,
  canFitDockSplit,
  dockLeaves,
  dockPaneCloseDirection,
  isEmptyTab,
  paneViewLabel,
  tabLabel,
  useWorkspaceStore,
  type DockSplitDirection,
  type WorkspaceView,
} from "@/features/workspace";
import { usePointerReorder } from "@/shared/hooks/usePointerReorder";
import {
  cn,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  IconButton,
  inwardSide,
  MenuTrigger,
  OverflowFadeText,
  OverlaySideProvider,
  Pressable,
} from "@/shared/ui";
import {
  ChevronDown,
  PanelBottomClose,
  PanelBottomDashed,
  PanelLeftClose,
  PanelRightClose,
  PanelRightDashed,
  PanelTopClose,
  Plus,
  X,
} from "lucide-react";
import { useCallback, useEffect, useRef } from "react";
import { ConnectedTabShape } from "./ConnectedTabShape";
import { shellMotionClass } from "./styles";
import { useTabPresenceMotion } from "./useTabPresenceMotion";
import { usePaneBounds, useTabStripFade } from "./useTabStripGeometry";
import { minimumForWorkspaceViews } from "./WorkspaceDockTree";
import { ViewIcon } from "./WorkspaceSurface";

/** Window tabs, joined to the pane below and sharing the titlebar band. */
export function WorkspaceTabStrip(props: {
  titlebarInsets?: { left: number; right: number; animate?: boolean };
  onOpen(view: WorkspaceView): void;
  onNewTab(): void;
  onCloseTab(id: string): void;
  onSplitPane(paneId: string, direction: DockSplitDirection): void;
  onClosePane(paneId: string): void;
}) {
  const layout = useWorkspaceStore((state) => state.layout);
  // A workspace with no tabs keeps one empty layout tab that is never shown.
  const tabs = layout.tabs.filter((tab) => !isEmptyTab(tab));
  const ref = useRef<HTMLElement>(null);
  const tabListRef = useRef<HTMLDivElement>(null);
  useTabPresenceMotion(tabListRef, true);
  useTabStripFade(tabListRef, true, tabs.length);
  const panes = dockLeaves(layout.root);
  const pane =
    panes.find((candidate) => candidate.id === layout.focusedPaneId) ??
    panes[0];
  const bounds = usePaneBounds(pane.id, layout.root);
  const ClosePaneIcon = {
    up: PanelTopClose,
    down: PanelBottomClose,
    left: PanelLeftClose,
    right: PanelRightClose,
  }[dockPaneCloseDirection(layout.root, pane.id) ?? "up"];
  const canSplit = panes.length < 4 && pane.views.length > 0;
  const fits = (direction: DockSplitDirection) =>
    canSplit &&
    canFitDockSplit(bounds, direction, minimumForWorkspaceViews(pane.views), {
      width: 280,
      height: 180,
    });
  const reorder = usePointerReorder({
    scope: "workspace-layout-tabs",
    animate: true,
    axis: "x",
    getDrag: (id) => {
      const tab = tabs.find((candidate) => candidate.id === id);
      return tab ? { id, label: tabLabel(tab) } : null;
    },
    onDrop: (drag, target, after) =>
      useWorkspaceStore.getState().moveTab(drag.id, target, after),
    onKeyboardMove: (id, direction) => {
      const index = tabs.findIndex((tab) => tab.id === id);
      const target = tabs[index + direction];
      if (target)
        useWorkspaceStore.getState().moveTab(id, target.id, direction > 0);
    },
  });
  const reorderRef = reorder.ref;
  const bindTabList = useCallback(
    (node: HTMLDivElement | null) => {
      tabListRef.current = node;
      reorderRef(node);
    },
    [reorderRef],
  );
  useEffect(() => {
    ref.current
      ?.querySelector('[aria-selected="true"]')
      ?.closest("[data-reorder-item]")
      ?.scrollIntoView?.({ block: "nearest", inline: "nearest" });
  }, [layout.activeTabId]);
  const select = (id: string) => {
    const view = useWorkspaceStore.getState().selectTab(id);
    if (view) props.onOpen(view);
  };
  return (
    <OverlaySideProvider value={inwardSide.top}>
      <header
        ref={ref}
        data-tab-position="top"
        className={cn(
          "kura-workspace-tabs flex min-w-0 shrink-0 border-charcoal-border bg-charcoal-workspace",
          props.titlebarInsets?.animate &&
            cn("transition-[padding]", shellMotionClass),
        )}
        style={{
          // The first shoulder starts in the strip's inset, next to the traffic
          // lights, but never closer to the edge than a rounded pane corner.
          paddingLeft: `max(12px, ${props.titlebarInsets?.left ?? 8}px - var(--tab-shoulder))`,
          paddingRight: 8 + (props.titlebarInsets?.right ?? 0),
        }}
        data-kura-window-titlebar-region={
          props.titlebarInsets ? "true" : undefined
        }
      >
        <div
          {...reorder}
          ref={bindTabList}
          role="tablist"
          aria-label="Window tabs"
          aria-orientation="horizontal"
          className={cn(
            "kura-workspace-tab-list flex min-w-0 flex-1 gap-1",
            "items-center overflow-x-auto overflow-y-hidden [scrollbar-width:none] [&::-webkit-scrollbar]:hidden",
          )}
        >
          {tabs.map((tab, index) => {
            const active = tab.id === layout.activeTabId,
              view = activeLayoutView(tab),
              label = tabLabel(tab),
              tabPanes = dockLeaves(tab.root);
            return (
              <Renameable
                key={tab.id}
                nameKey={`layout-tab:${tab.id}`}
                automatic={tabLabel({ ...tab, title: undefined })}
                customName={tab.title}
                onRename={(name) =>
                  useWorkspaceStore.getState().renameTab(tab.id, name ?? "")
                }
                resetLabel="Use active pane title"
              >
                <div
                  data-active={active}
                  data-reorder-item={tab.id}
                  data-reorder-preview="true"
                  data-kura-window-drag-block="true"
                  className={cn(
                    "kura-workspace-tab group/tab flex items-center rounded-md border text-xs transition-colors",
                    "duration-150 select-none focus-within:ring-1",
                    "focus-within:ring-cream-muted/50",
                    active
                      ? "border-charcoal-border/70 bg-charcoal-card text-cream-bright shadow-sm"
                      : "border-transparent text-cream-muted hover:bg-charcoal-card/40 hover:text-cream",
                  )}
                >
                  <ConnectedTabShape />
                  <Pressable
                    role="tab"
                    aria-selected={active}
                    tabIndex={active ? 0 : -1}
                    title={label}
                    data-reorder-handle="true"
                    className="flex h-full min-w-0 flex-1 items-center justify-start gap-1.5 overflow-hidden pl-2 pr-1"
                    onClick={() => select(tab.id)}
                    onKeyDown={(event) => {
                      const next =
                        event.key === "ArrowRight"
                          ? (index + 1) % tabs.length
                          : event.key === "ArrowLeft"
                            ? (index + tabs.length - 1) % tabs.length
                            : event.key === "Home"
                              ? 0
                              : event.key === "End"
                                ? tabs.length - 1
                                : -1;
                      if (
                        event.altKey ||
                        event.ctrlKey ||
                        event.metaKey ||
                        next < 0
                      )
                        return;
                      event.preventDefault();
                      select(tabs[next].id);
                      requestAnimationFrame(() =>
                        ref.current
                          ?.querySelectorAll<HTMLButtonElement>('[role="tab"]')
                          .item(next)
                          ?.focus(),
                      );
                    }}
                  >
                    <ViewIcon view={view} />
                    <OverflowFadeText className="min-w-0 flex-1 overflow-hidden whitespace-nowrap">
                      {label}
                    </OverflowFadeText>
                    {tabPanes.length > 1 ? (
                      <span
                        aria-hidden="true"
                        className="shrink-0 text-[10px] text-cream-muted tabular-nums"
                      >
                        {tabPanes.length}
                      </span>
                    ) : null}
                  </Pressable>
                  {tabPanes.length > 1 ? (
                    <DropdownMenu modal={false}>
                      <MenuTrigger
                        iconOnly
                        size="xs"
                        className="mr-0.5"
                        label={`Show panes in ${label}`}
                        icon={<ChevronDown className="size-3" size={12} />}
                      />
                      <DropdownMenuContent
                        align="start"
                        width="md"
                        className="max-w-[calc(100vw-24px)]"
                      >
                        {tabPanes.map((item) => {
                          const itemView = item.views[0];
                          if (!itemView) return null;
                          const focused = item.id === tab.focusedPaneId;
                          return (
                            <div
                              key={item.id}
                              className="flex items-center gap-1"
                            >
                              <DropdownMenuItem
                                className={cn(
                                  "min-w-0 flex-1",
                                  focused &&
                                    "bg-charcoal-active text-cream-bright data-highlighted:bg-charcoal-active",
                                )}
                                aria-current={focused ? "true" : undefined}
                                onSelect={() => {
                                  useWorkspaceStore
                                    .getState()
                                    .focusView(itemView.id);
                                  props.onOpen(itemView);
                                }}
                              >
                                <ViewIcon view={itemView} />
                                <span className="min-w-0 flex-1 truncate">
                                  {paneViewLabel(itemView)}
                                </span>
                              </DropdownMenuItem>
                              <DropdownMenuItem
                                aria-label={`Close pane ${paneViewLabel(itemView)}`}
                                className="w-8 shrink-0 justify-center px-0"
                                onSelect={() =>
                                  useWorkspaceStore
                                    .getState()
                                    .closeView(itemView.id)
                                }
                              >
                                <X size={14} aria-hidden />
                              </DropdownMenuItem>
                            </div>
                          );
                        })}
                      </DropdownMenuContent>
                    </DropdownMenu>
                  ) : (
                    <IconButton
                      size="xs"
                      tooltip={false}
                      label={`Close tab ${label}`}
                      title={`Close tab ${label}`}
                      className="kura-workspace-tab-close mr-0.5"
                      onClick={() => props.onCloseTab(tab.id)}
                    >
                      <X className="size-3.5" size={14} />
                    </IconButton>
                  )}
                </div>
              </Renameable>
            );
          })}
          <IconButton
            className="kura-workspace-new-tab"
            size="xs"
            tooltip={false}
            label="New tab"
            onClick={props.onNewTab}
          >
            <Plus className="size-3.5" />
          </IconButton>
        </div>
        <div
          className="kura-workspace-tab-actions"
          data-kura-window-drag-block="true"
        >
          <IconButton
            size="xs"
            tooltip={false}
            disabled={!fits("right")}
            label="Split right"
            title="Split right"
            onClick={() => props.onSplitPane(pane.id, "right")}
          >
            <PanelRightDashed className="size-4" size={16} />
          </IconButton>
          <IconButton
            size="xs"
            tooltip={false}
            disabled={!fits("down")}
            label="Split down"
            title="Split down"
            onClick={() => props.onSplitPane(pane.id, "down")}
          >
            <PanelBottomDashed className="size-4" size={16} />
          </IconButton>
          <IconButton
            size="xs"
            tooltip={false}
            disabled={panes.length <= 1}
            label="Close pane"
            title="Close pane"
            onClick={() => props.onClosePane(pane.id)}
          >
            <ClosePaneIcon className="size-4" size={16} />
          </IconButton>
        </div>
      </header>
    </OverlaySideProvider>
  );
}
