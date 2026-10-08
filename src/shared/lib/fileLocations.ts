/** Local paths and paired LAN devices are the only external Files locations. */
export function isRetiredCloudLocation(
  path: string,
  legacyMountRoot?: string,
): boolean {
  const normalized = path.trim().replace(/\\/g, "/").replace(/\/+$/, "");
  if (normalized.startsWith("kura://device/")) return false;
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
  const mount = legacyMountRoot?.replace(/\\/g, "/").replace(/\/+$/, "");
  return (
    Boolean(
      mount && (normalized === mount || normalized.startsWith(`${mount}/`)),
    ) || /(?:^|\/)\.misty\/mnt(?:\/|$)/.test(normalized)
  );
}
