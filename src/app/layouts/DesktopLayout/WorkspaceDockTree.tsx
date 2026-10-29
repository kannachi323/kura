import {
  dockLeaves,
  panePlacement,
  StablePaneLayout,
  surfaceMinimumSize,
  useWorkspaceStore,
  type DockDropZone,
  type WorkspaceDockNode,
  type WorkspacePane,
  type WorkspaceView,
} from "@/features/workspace";
import { usePointerDropTarget } from "@/shared/hooks/usePointerReorder";
import { cn } from "@/shared/ui";
import { memo, useEffect, useRef } from "react";
import {
  Panel,
  PanelGroup,
  PanelResizeHandle,
  type ImperativePanelGroupHandle,
} from "react-resizable-panels";
import { WorkspacePaneControls } from "./WorkspacePaneControls";
import { EmptyWorkspacePane, WorkspaceSurface } from "./WorkspaceSurface";

export interface WorkspaceDockTreeProps {
  workspaceActive?: boolean;
  node: WorkspaceDockNode;
  focusedPaneId: string;
  onOpen: (view: WorkspaceView) => void;
  onClosePane: (paneId: string) => void;
  onResizeSplit: (splitId: string, ratio: number) => void;
}

export function WorkspaceDockTree(props: WorkspaceDockTreeProps) {
  const panes = dockLeaves(props.node).map((pane) => ({
    id: pane.id,
    content: <DockLeafView {...props} pane={pane} />,
  }));
  return (
    <StablePaneLayout panes={panes}>
      <DockLayoutNode {...props} />
    </StablePaneLayout>
  );
}

function DockLayoutNode(props: WorkspaceDockTreeProps) {
  if (props.node.type === "leaf")
    return (
      <div
        data-pane-layout-slot={props.node.id}
        className="h-full min-h-0 w-full min-w-0"
      />
    );
  return <DockSplitView key={props.node.id} {...props} node={props.node} />;
}

function DockSplitView(
  props: WorkspaceDockTreeProps & {
    node: Extract<WorkspaceDockNode, { type: "split" }>;
  },
) {
  const persistTimerRef = useRef<number | null>(null);
  const pendingRatioRef = useRef(props.node.ratio);
  const panelGroupRef = useRef<ImperativePanelGroupHandle | null>(null);
  useEffect(() => {
    const handle = panelGroupRef.current;
    const layout = handle?.getLayout();
    if (!handle || layout?.length !== 2) return;
    const desired = props.node.ratio * 100;
    if (Math.abs((layout[0] ?? desired) - desired) < 0.1) return;
    pendingRatioRef.current = props.node.ratio;
    handle.setLayout([desired, 100 - desired]);
  }, [props.node.ratio]);
  useEffect(
    () => () => {
      if (persistTimerRef.current !== null)
        window.clearTimeout(persistTimerRef.current);
    },
    [],
  );
  return (
    <PanelGroup
      ref={panelGroupRef}
      id={props.node.id}
      direction={props.node.direction}
      className="h-full min-h-0 w-full min-w-0"
      onLayout={(sizes) => {
        pendingRatioRef.current = (sizes[0] ?? 50) / 100;
        if (persistTimerRef.current !== null)
          window.clearTimeout(persistTimerRef.current);
        persistTimerRef.current = window.setTimeout(() => {
          persistTimerRef.current = null;
          props.onResizeSplit(props.node.id, pendingRatioRef.current);
        }, 120);
      }}
    >
      <Panel
        id={`${props.node.id}:first`}
        order={1}
        defaultSize={props.node.ratio * 100}
        minSize={15}
        className="min-h-0 min-w-0"
      >
        <DockLayoutNode {...props} node={props.node.first} />
      </Panel>
      <PanelResizeHandle
        aria-label={
          props.node.direction === "horizontal"
            ? "Resize panes horizontally"
            : "Resize panes vertically"
        }
        className={cn(
          "relative z-20 bg-charcoal-border transition-colors before:absolute",
          "hover:bg-charcoal-active focus-visible:outline-none focus-visible:ring-2",
          "focus-visible:ring-cream-muted/60 data-[resize-handle-active]:bg-charcoal-active",
          props.node.direction === "horizontal"
            ? "w-px cursor-col-resize before:inset-y-0 before:-inset-x-[5px]"
            : "h-px cursor-row-resize before:inset-x-0 before:-inset-y-[5px]",
        )}
      />
      <Panel
        id={`${props.node.id}:second`}
        order={2}
        defaultSize={(1 - props.node.ratio) * 100}
        minSize={15}
        className="min-h-0 min-w-0"
      >
        <DockLayoutNode {...props} node={props.node.second} />
      </Panel>
    </PanelGroup>
  );
}

/**
 * One open view. Every open view stays mounted, so a store change elsewhere
 * leaves an untouched view's object, and therefore its page, alone.
 */
const MountedWorkspaceView = memo(function MountedWorkspaceView(props: {
  view: WorkspaceView;
  active: boolean;
}) {
  return <WorkspaceSurface view={props.view} active={props.active} />;
});

function DockLeafView(props: WorkspaceDockTreeProps & { pane: WorkspacePane }) {
  const { pane } = props;
  const sectionRef = useRef<HTMLElement | null>(null);
  const activeView =
    pane.views.find((view) => view.id === pane.activeViewId) ?? pane.views[0];
  const focused =
    props.workspaceActive !== false && pane.id === props.focusedPaneId;
  const multiPane =
    dockLeaves(useWorkspaceStore.getState().layout.root).length > 1;

  // A pane dragged by its handle moves to an edge or swaps places.
  usePointerDropTarget(sectionRef, {
    scope: "workspace-panes",
    hit(x, y, drag) {
      if (!sectionRef.current) return null;
      const rect = sectionRef.current.getBoundingClientRect();
      const root = useWorkspaceStore.getState().layout.root;
      const leafRects = dockLeaves(root)
        .map((leaf) =>
          document
            .querySelector<HTMLElement>(
              `[data-workspace-pane="${CSS.escape(leaf.id)}"]`,
            )
            ?.getBoundingClientRect(),
        )
        .filter((value): value is DOMRect => Boolean(value));
      const left = Math.min(...leafRects.map((value) => value.left));
      const top = Math.min(...leafRects.map((value) => value.top));
      const placement = panePlacement(root, drag.id, x, y, {
        x: left,
        y: top,
        width: Math.max(...leafRects.map((value) => value.right)) - left,
        height: Math.max(...leafRects.map((value) => value.bottom)) - top,
      });
      if (!placement) return null;
      const projected = placement.bounds;
      return {
        previewRect: new DOMRect(
          projected.x,
          projected.y,
          projected.width,
          projected.height,
        ),
        id: JSON.stringify([placement.target, placement.zone]),
        after: placement.zone === "right" || placement.zone === "down",
        rect,
        axis:
          placement.zone === "left" || placement.zone === "right" ? "x" : "y",
      };
    },
    drop(drag, hit) {
      const store = useWorkspaceStore.getState();
      const [target, zone] = JSON.parse(hit.id) as [string, DockDropZone];
      if (zone === "center") store.swapPanes(drag.id, target);
      else store.movePane(drag.id, zone, target || undefined);
    },
  });

  const mountedViews = [...pane.views].sort((a, b) => a.id.localeCompare(b.id));
  return (
    <section
      ref={sectionRef}
      className={cn(
        "group/pane relative grid h-full min-h-0 min-w-0 grid-rows-[minmax(0,1fr)]",
        "overflow-hidden bg-charcoal-bg focus:outline-none focus-visible:ring-2",
        "focus-visible:ring-inset focus-visible:ring-cream-muted/50",
      )}
      data-workspace-pane={pane.id}
      tabIndex={-1}
      aria-label={
        activeView ? `${activeView.title} pane` : "Empty workspace pane"
      }
      onPointerDown={() => {
        if (!activeView) {
          useWorkspaceStore.getState().focusPane(pane.id);
          return;
        }
        if (!focused) props.onOpen(activeView);
      }}
    >
      <div className="min-h-0 min-w-0 overflow-hidden">
        {activeView ? (
          mountedViews.map((view) => {
            const isActive =
              props.workspaceActive !== false && view.id === activeView.id;
            return (
              <div
                key={view.id}
                className={cn(
                  "h-full min-h-0 w-full",
                  view.id === activeView.id ? "block" : "hidden",
                )}
                aria-hidden={view.id !== activeView.id}
              >
                <MountedWorkspaceView
                  view={view}
                  active={isActive && (focused || !multiPane)}
                />
              </div>
            );
          })
        ) : (
          <EmptyWorkspacePane />
        )}
      </div>
      {focused && multiPane ? (
        <div
          data-pane-focus
          aria-hidden
          className="pointer-events-none absolute inset-0 z-20 ring-1 ring-inset ring-cream-muted/50"
        />
      ) : null}
      <WorkspacePaneControls
        pane={pane}
        onClose={() => props.onClosePane(pane.id)}
      />
    </section>
  );
}

export function minimumForWorkspaceViews(views: WorkspaceView[]): {
  width: number;
  height: number;
} {
  return views.reduce(
    (minimum, view) => {
      const next = surfaceMinimumSize[view.surfaceId];
      return {
        width: Math.max(minimum.width, next.width),
        height: Math.max(minimum.height, next.height),
      };
    },
    { width: 280, height: 180 },
  );
}
