import type { TransferRecord } from "@/native/ipc";

/**
 * The operation Cmd+Z undoes: the newest undoable one queued since
 * `sinceMs` (this run of Kura). Older ones keep their Undo button in
 * Transfers, but a keystroke shouldn't revert yesterday's rename.
 */
export function newestUndoableTransfer(
  rows: readonly TransferRecord[],
  sinceMs = 0,
): TransferRecord | null {
  return (
    rows
      .filter(
        (row) =>
          row.undoable &&
          row.undoTokenId > 0 &&
          row.status === "completed" &&
          row.queuedAtMs >= sinceMs,
      )
      .sort(
        (left, right) =>
          transferRecencyMs(right) - transferRecencyMs(left) ||
          right.id - left.id,
      )[0] ?? null
  );
}

export function transferRecencyMs(row: TransferRecord): number {
  return row.completedAtMs || row.startedAtMs || row.queuedAtMs || row.id;
}

export function transferTypeLabel(
  type: TransferRecord["transferType"],
): string {
  return type.charAt(0).toUpperCase() + type.slice(1);
}
