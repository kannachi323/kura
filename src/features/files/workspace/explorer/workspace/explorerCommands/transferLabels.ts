import type { TransferRecord } from "@/native/ipc";

export function newestUndoableTransfer(
  rows: readonly TransferRecord[],
): TransferRecord | null {
  return (
    rows
      .filter(
        (row) =>
          row.undoable && row.undoTokenId > 0 && row.status === "completed",
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
