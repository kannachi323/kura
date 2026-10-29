import { formatBytes } from "@/features/file-ui";
import type { DirectoryListing } from "@/native/ipc";
import { useEffect, useState } from "react";
import { explorerVolumeSpace } from "../../native";

/**
 * "245.3 GB available" for a local folder's volume, refreshed whenever the
 * listing changes (a copy or delete changes free space).
 */
export function useVolumeSpaceLabel(
  listing: DirectoryListing | null,
): string | null {
  const [label, setLabel] = useState<string | null>(null);
  const path =
    listing?.location.kind === "local" && !listing.path.startsWith("kura://")
      ? listing.path
      : null;
  useEffect(() => {
    if (!path) {
      setLabel(null);
      return;
    }
    let cancelled = false;
    explorerVolumeSpace(path)
      .then((space) => {
        if (!cancelled)
          setLabel(`${formatBytes(space.availableBytes)} available`);
      })
      .catch(() => {
        if (!cancelled) setLabel(null);
      });
    return () => {
      cancelled = true;
    };
  }, [path, listing]);
  return label;
}
