import type { OperationDescriptor, TransferRecord } from "@/native/ipc";

export const transfersPath = "kura-transfers://history";
export type TransferSection = "all" | "active" | "completed" | "failed";
export const transferSections = [
  { value: "all", label: "All" },
  { value: "active", label: "Active" },
  { value: "completed", label: "Completed" },
  { value: "failed", label: "Failed" },
];
export function isActiveTransfer(row: TransferRecord) {
  return [
    "queued",
    "pending",
    "in_progress",
    "waiting_for_resolution",
  ].includes(row.status);
}
export function transferStatus(row: TransferRecord) {
  if (row.paused && isActiveTransfer(row)) return "Paused";
  return {
    queued: "Queued",
    pending: "Pending",
    in_progress: "Transferring",
    waiting_for_resolution: "Needs attention",
    completed: "Completed",
    failed: "Failed",
    canceled: "Canceled",
    skipped: "Skipped",
    interrupted: "Interrupted",
  }[row.status];
}
export function transferActions(
  row: TransferRecord,
  operation?: OperationDescriptor,
) {
  const active = isActiveTransfer(row);
  const liveActive = Boolean(
    operation &&
    ["queued", "in_progress", "waiting_for_resolution"].includes(
      operation.status,
    ),
  );
  return {
    cancel: active && liveActive && Boolean(operation?.cancelable),
    pause:
      active &&
      liveActive &&
      !operation?.paused &&
      row.status !== "waiting_for_resolution",
    // A running copy cleans up its partial destination before it can restart.
    resume:
      active && Boolean(operation?.paused) && operation?.status === "queued",
    retry: row.status === "failed" && row.retryable,
    undo: row.status === "completed" && row.undoable && row.undoTokenId > 0,
  };
}
/**
 * The folder an endpoint is in, short enough for a table cell:
 * "~/Documents/Reports", or "…/Reports/2026" when it is deep.
 */
export function shortFolderPath(path: string, homeDir: string): string {
  if (!path) return "";
  const separator = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  let folder = separator > 0 ? path.slice(0, separator) : path;
  const home = homeDir.replace(/[\\/]+$/, "");
  if (home && (folder === home || folder.startsWith(`${home}/`)))
    folder = `~${folder.slice(home.length)}`;
  const parts = folder.split(/[\\/]/).filter(Boolean);
  return parts.length > 3 ? `…/${parts.slice(-2).join("/")}` : folder;
}

export function transferEndpoints(row: TransferRecord) {
  return {
    source:
      row.localSourcePath ||
      [row.remoteSourceName, row.remoteSourcePath].filter(Boolean).join(":"),
    destination:
      row.localDestPath ||
      [row.remoteDestName, row.remoteDestPath].filter(Boolean).join(":"),
  };
}
