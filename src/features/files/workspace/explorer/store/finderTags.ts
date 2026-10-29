import type { FileEntry, FinderTag } from "@/native/ipc";
import { isMacPlatform } from "@/shared/platform/tauri";
import { useEffect } from "react";
import { create } from "zustand";
import { explorerReadTags } from "../../native";

/**
 * Finder tags, read on demand for the rows on screen. Reading them for a
 * whole listing would cost one getxattr per entry, about ten times an
 * lstat, so they are not part of directory listings.
 */
export const useFinderTagStore = create<{
  tags: Record<string, FinderTag[]>;
}>(() => ({ tags: {} }));

const requested = new Set<string>();
const pending = new Set<string>();
let flushTimer: ReturnType<typeof setTimeout> | null = null;

/** The tags on `entry`, loading them if needed; undefined until loaded. */
export function useEntryTags(entry: FileEntry): FinderTag[] | undefined {
  const taggable =
    entry.location.kind === "local" && !entry.isDeleted && isMacPlatform();
  const tags = useFinderTagStore((state) => state.tags[entry.path]);
  const loaded = tags !== undefined;
  useEffect(() => {
    if (taggable && !loaded && !requested.has(entry.path))
      requestTags(entry.path);
  }, [entry.path, loaded, taggable]);
  return tags;
}

export function tagsForPath(path: string): FinderTag[] {
  return useFinderTagStore.getState().tags[path] ?? [];
}

/** Drops cached tags under `folder` so rows on screen read them again. */
export function forgetFolderTags(folder: string): void {
  const prefix = folder.endsWith("/") ? folder : `${folder}/`;
  forgetTags(
    Object.keys(useFinderTagStore.getState().tags).filter((path) =>
      path.startsWith(prefix),
    ),
  );
}

export function forgetTags(paths: string[]): void {
  if (paths.length === 0) return;
  for (const path of paths) requested.delete(path);
  useFinderTagStore.setState((state) => {
    const tags = { ...state.tags };
    for (const path of paths) delete tags[path];
    return { tags };
  });
}

function requestTags(path: string): void {
  requested.add(path);
  pending.add(path);
  // Batch the rows that mount together into one IPC call.
  flushTimer ??= setTimeout(() => void flush(), 16);
}

async function flush(): Promise<void> {
  flushTimer = null;
  const paths = [...pending];
  pending.clear();
  try {
    const results = await explorerReadTags(paths);
    useFinderTagStore.setState((state) => {
      const tags = { ...state.tags };
      paths.forEach((path, index) => {
        tags[path] = results[index] ?? [];
      });
      return { tags };
    });
  } catch {
    for (const path of paths) requested.delete(path);
  }
}
