/**
 * Locations that no longer exist: remote URLs and the cloud mounts early Kura
 * used. Cloud accounts now live in ordinary folders under
 * `~/.kura/providers`, which are not retired.
 */
export function isRetiredCloudLocation(path: string): boolean {
  const normalized = path.trim().replace(/\\/g, "/").replace(/\/+$/, "");
  if (normalized.startsWith("kura://device/")) return false;
  // Items with a Finder tag (the sidebar's Tags section).
  if (normalized.startsWith("kura://tag/")) return false;
  if (normalized.startsWith("kura-remotes://")) return true;
  if (
    normalized.includes("://") &&
    ![
      "kura://local",
      "kura://recent",
      "kura://starred",
      "kura://trash",
      "kura://library",
      "kura-transfers://history",
    ].includes(normalized)
  )
    return true;
  return /(?:^|\/)\.kura\/mnt(?:\/|$)/.test(normalized);
}
