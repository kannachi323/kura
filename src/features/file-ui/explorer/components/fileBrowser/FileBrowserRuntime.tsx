import { createContext, useContext, type ComponentType } from "react";
import type { FileEntry, FinderTag } from "@/native/ipc";

export interface FileBrowserRuntime {
  thumbnailPreviewsEnabled: boolean;
  compactModeEnabled: boolean;
  prewarmThumbnails(entries: FileEntry[]): void;
  requestThumbnail(
    entry: FileEntry,
    maxDimension: number,
    subscriber: (url: string | null) => void,
  ): () => void;
  Error: ComponentType<{ error: string; paneId: string }>;
  /** Finder tags for a row, loaded on demand. Optional: pickers omit it. */
  useEntryTags?(entry: FileEntry): FinderTag[] | undefined;
}

const Runtime = createContext<FileBrowserRuntime | null>(null);
export const FileBrowserRuntimeProvider = Runtime.Provider;
export function useFileBrowserRuntime(): FileBrowserRuntime {
  const runtime = useContext(Runtime);
  if (!runtime) throw new Error("FileBrowser requires its owning app runtime.");
  return runtime;
}
