import { Button } from "@/shared/ui";
import { isMacPlatform } from "@/shared/platform/tauri";
import { useEffect, useState } from "react";
import { explorerOpenPath, explorerTrashFullyVisible } from "../../native";

/**
 * In the Trash view: says when Kura can only show what it trashed itself,
 * because listing the macOS Trash needs Full Disk Access.
 */
export function TrashNotice() {
  const [fullyVisible, setFullyVisible] = useState(true);
  useEffect(() => {
    if (!isMacPlatform()) return;
    let cancelled = false;
    explorerTrashFullyVisible()
      .then((visible) => {
        if (!cancelled) setFullyVisible(visible);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, []);
  if (fullyVisible) return null;
  return (
    <div className="flex items-center gap-3 border-b border-charcoal-border px-3 py-2 text-xs text-cream-muted">
      <span className="min-w-0 flex-1">
        Showing items Kura moved to the Trash. To see everything in it, give
        Kura Full Disk Access.
      </span>
      <Button
        size="sm"
        variant="ghost"
        onClick={() =>
          void explorerOpenPath(
            "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles",
          )
        }
      >
        Open Privacy Settings
      </Button>
    </div>
  );
}
