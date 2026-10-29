import { useMemo } from "react";
import { FolderX, Lock } from "lucide-react";
import { Button } from "@/shared/ui";
import { isMacPlatform } from "@/shared/platform/tauri";
import { explorerOpenPath } from "../../native";
import { useExplorerStore } from "../store";
import { useEntryTags } from "../store/finderTags";
import { SystemErrorActivity } from "@/features/activity";
import {
  selectAppearancePreferences,
  useSettingsStore,
} from "@/features/settings";
import {
  FileBrowserView,
  FileBrowserRuntimeProvider,
  type FileBrowserProps,
} from "@/features/file-ui";
import {
  prewarmGridThumbnails,
  requestGridThumbnail,
} from "./fileBrowser/gridThumbnails";
export type {
  FileBrowserProps,
  GridThumbnailJob,
  FileTableColumn,
  FileTableColumnWidths,
  GridThumbnailSubscriber,
  FileBrowserDragItem,
} from "@/features/file-ui";

const permissionError =
  /operation not permitted|permission denied|os error (1|13)\b/i;
const missingError = /no such file or directory|os error 2\b/i;

function ErrorView({ error, paneId }: { error: string; paneId: string }) {
  const denied = permissionError.test(error);
  if (denied || missingError.test(error)) {
    const mac = isMacPlatform();
    return (
      <div className="grid h-full place-content-center justify-items-center gap-3 p-8 text-center">
        <span className="grid size-11 place-items-center rounded-full bg-charcoal-hover text-cream-muted">
          {denied ? <Lock size={20} /> : <FolderX size={20} />}
        </span>
        <h2 className="text-sm font-semibold text-cream">
          {denied
            ? "Kura doesn't have permission to open this folder"
            : "This folder no longer exists"}
        </h2>
        <p className="max-w-sm text-xs text-cream-muted">
          {denied
            ? mac
              ? "macOS protects this folder. Give Kura Full Disk Access in System Settings › Privacy & Security, then try again."
              : "Your account can't read this folder. Ask its owner for access, then try again."
            : "It may have been moved, renamed or deleted by another app."}
        </p>
        <div className="flex gap-2">
          {denied && mac ? (
            <Button
              size="sm"
              onClick={() =>
                void explorerOpenPath(
                  "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles",
                )
              }
            >
              Open Privacy Settings
            </Button>
          ) : null}
          <Button
            size="sm"
            variant="ghost"
            onClick={() => {
              const store = useExplorerStore.getState();
              const failed = store.panes[paneId]?.errorPath;
              if (!failed) return void store.refreshPane(paneId);
              const separator = Math.max(
                failed.lastIndexOf("/"),
                failed.lastIndexOf("\\"),
              );
              void store.loadPane(
                paneId,
                denied ? failed : failed.slice(0, separator) || "/",
                "replace",
              );
            }}
          >
            {denied ? "Try Again" : "Go to Enclosing Folder"}
          </Button>
        </div>
      </div>
    );
  }
  return (
    <SystemErrorActivity
      error={error}
      scope={`files:browser:${paneId}`}
      title="Files could not be loaded"
    />
  );
}
export function FileBrowser(props: FileBrowserProps) {
  const thumbnailPreviewsEnabled = useSettingsStore(
    (state) =>
      selectAppearancePreferences(state.settings?.document)
        .thumbnailPreviewsEnabled,
  );
  const compactModeEnabled = useSettingsStore(
    (state) =>
      selectAppearancePreferences(state.settings?.document).compactModeEnabled,
  );
  const runtime = useMemo(
    () => ({
      thumbnailPreviewsEnabled,
      compactModeEnabled,
      prewarmThumbnails: prewarmGridThumbnails,
      requestThumbnail: requestGridThumbnail,
      Error: ErrorView,
      useEntryTags,
    }),
    [thumbnailPreviewsEnabled, compactModeEnabled],
  );
  return (
    <FileBrowserRuntimeProvider value={runtime}>
      <FileBrowserView {...props} />
    </FileBrowserRuntimeProvider>
  );
}
