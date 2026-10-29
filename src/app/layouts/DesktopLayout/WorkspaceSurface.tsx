import { FilesPage } from "@/features/files/workspace";
import { TransfersPage } from "@/features/files/transfers/TransfersPage";
import {
  surfaceTitles,
  useWorkspaceStore,
  type WorkspaceSurfaceId,
  type WorkspaceView,
} from "@/features/workspace";
import { ArrowRightLeft, FolderOpen } from "lucide-react";
import { RenderErrorBoundary } from "../RenderErrorBoundary";

export const surfaceIcons: Record<WorkspaceSurfaceId, typeof FolderOpen> = {
  files: FolderOpen,
  transfers: ArrowRightLeft,
};

export function WorkspaceSurface({
  view,
  active,
}: {
  view: WorkspaceView;
  active: boolean;
}) {
  const commit = () => useWorkspaceStore.getState().commitPlaceholder(view.id);
  return (
    <RenderErrorBoundary key={`${view.id}:${view.surfaceId}`} scope="tab">
      <div className="contents" onInputCapture={commit} onClickCapture={commit}>
        {view.surfaceId === "files" ? (
          <FilesPage
            embedded
            active={active}
            workspaceId={view.id}
            workspaceTitle={view.title}
          />
        ) : (
          <TransfersPage />
        )}
      </div>
    </RenderErrorBoundary>
  );
}

export function ViewIcon({
  view,
  className,
}: {
  view: WorkspaceView | null | undefined;
  className?: string;
}) {
  const Icon = surfaceIcons[view?.surfaceId ?? "files"];
  return (
    <Icon
      className={className ?? "size-4 shrink-0"}
      size={16}
      aria-label={view ? surfaceTitles[view.surfaceId] : undefined}
      aria-hidden={view ? undefined : true}
    />
  );
}

export function EmptyWorkspacePane() {
  return (
    <div className="grid h-full place-items-center bg-charcoal-bg px-6 text-center">
      <p className="m-0 text-base font-medium text-cream">
        A little quiet here...
      </p>
    </div>
  );
}
