import { lazy, Suspense } from "react";
import { ExplorerLoadingShell } from "./components/ExplorerLoadingShell";

export * from "./components/FileBrowser";
export * from "./components/GlobalPreview";

export * from "./drag/ExplorerDragContext";
export * from "./drag/ExplorerDropTarget";
export * from "./store";
export * from "./utils/globalSearch";
export * from "./utils/librarySearch";
export * from "./utils/searchNavigation";

const loadDesktopFilesPage = () => import("./workspace");
const DesktopFilesPage = lazy(loadDesktopFilesPage);

export default function FilesPage(props: {
  embedded?: boolean;
  active?: boolean;
  workspaceId?: string;
  workspaceTitle?: string;
}) {
  return (
    <Suspense fallback={<ExplorerLoadingShell embedded={props.embedded} />}>
      <DesktopFilesPage {...props} />
    </Suspense>
  );
}
