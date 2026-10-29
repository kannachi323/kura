import type { TransferRecord } from "@/native/ipc";
import { describe, expect, it } from "vitest";
import { newestUndoableTransfer } from "./transferLabels";

const row = (id: number, queuedAtMs: number): TransferRecord =>
  ({
    id,
    status: "completed",
    undoable: true,
    undoTokenId: id,
    queuedAtMs,
    startedAtMs: queuedAtMs,
    completedAtMs: queuedAtMs,
  }) as TransferRecord;

describe("newestUndoableTransfer", () => {
  it("picks the newest undoable operation", () => {
    expect(newestUndoableTransfer([row(1, 100), row(2, 200)])?.id).toBe(2);
  });

  it("ignores operations from before this run of Kura", () => {
    // Yesterday's rename keeps its Undo button in Transfers, but Cmd+Z
    // after a relaunch must not silently revert it.
    expect(newestUndoableTransfer([row(1, 100)], 500)).toBeNull();
    expect(newestUndoableTransfer([row(1, 100), row(2, 600)], 500)?.id).toBe(2);
  });
});
