import type { FileEntry } from "@/native/ipc";
import { useEffect, useState } from "react";
import { quickLookEvent } from "./fileListKeyboard";
import { GlobalPreviewDialogView } from "./globalPreview/GlobalPreviewDialogView";
import { useHostPreviewRuntime } from "./globalPreview/hostPreviewRuntime";

/**
 * Quick Look: Space (or Cmd+Y) previews the selected file in a large
 * dialog, whether or not the inspector is showing. The same key closes it.
 */
export function QuickLookHost(props: { onSaved?: () => void }) {
  const runtime = useHostPreviewRuntime();
  const [entry, setEntry] = useState<FileEntry | null>(null);
  useEffect(() => {
    const toggle = (event: Event) => {
      const next = (event as CustomEvent<{ entry: FileEntry }>).detail.entry;
      if (next.kind === "folder" || next.kind === "symlink" || next.isDeleted)
        return;
      setEntry((current) => (current?.path === next.path ? null : next));
    };
    window.addEventListener(quickLookEvent, toggle);
    return () => window.removeEventListener(quickLookEvent, toggle);
  }, []);
  if (!entry) return null;
  return (
    <GlobalPreviewDialogView
      runtime={runtime}
      source={{
        path: entry.path,
        name: entry.name,
        extension: entry.extension,
        mimeType: entry.mimeType,
        sizeBytes: entry.sizeBytes,
        modifiedMs: entry.modifiedMs,
        createdMs: entry.createdMs,
        originalName: entry.name,
        readonly: entry.readonly,
        remote: entry.location.kind !== "local",
        peer: entry.location.kind === "peer_device",
      }}
      onClose={() => setEntry(null)}
      onSaved={() => props.onSaved?.()}
    />
  );
}
