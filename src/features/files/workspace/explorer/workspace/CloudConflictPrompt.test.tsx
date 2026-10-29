import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { cloudResolveConflict } from "@/native/cloud";
import type { OperationQueueSnapshot } from "@/native/ipc";
import { operationQueueResolveConflict } from "@/native/transfers-tools";
import { useOperationQueueStore } from "../store";
import { FileOperationConflictDialog } from "./FileOperationConflictDialog";

vi.mock("@/native/transfers-tools", () => ({
  operationQueueResolveConflict: vi.fn(),
}));
vi.mock("@/native/cloud", () => ({
  cloudResolveConflict: vi.fn(),
}));

const conflict = {
  open: true,
  operationId: 9,
  batchId: 3,
  applyToBatch: true,
  supportsReplace: true,
  supportsKeepBoth: true,
  selectedPolicy: "replace",
  title: "Upload notes.md",
  sourceLabel: "work:/notes.md",
  targetLabel: "work:/notes.md",
  compareLocalPath: "/Users/me/.kura/providers/work/notes.md",
  compareCloudPath: "/Users/me/.kura/.cache/cloud-conflicts/work/x/notes.md",
} as const;
const closed = {
  conflictDialog: { ...conflict, open: false },
} as OperationQueueSnapshot;

beforeEach(() => {
  vi.clearAllMocks();
  useOperationQueueStore.setState({
    snapshot: { conflictDialog: conflict } as OperationQueueSnapshot,
  });
});

describe("cloud upload conflicts", () => {
  it("offers the cloud choices and keeps mine through the queue", async () => {
    vi.mocked(operationQueueResolveConflict).mockResolvedValue(closed);
    render(<FileOperationConflictDialog />);
    expect(
      screen.getByText("“notes.md” changed in the cloud too"),
    ).toBeTruthy();
    expect(
      screen.getByRole("button", { name: "Compare side by side" }),
    ).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Keep mine" }));
    await waitFor(() =>
      expect(operationQueueResolveConflict).toHaveBeenCalledWith(
        9,
        "replace",
        false,
      ),
    );
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
  });

  it("uses the cloud version through Kuragine", async () => {
    vi.mocked(cloudResolveConflict).mockResolvedValue(closed);
    render(<FileOperationConflictDialog />);
    fireEvent.click(screen.getByRole("button", { name: "Use cloud version" }));
    await waitFor(() =>
      expect(cloudResolveConflict).toHaveBeenCalledWith(
        conflict.compareLocalPath,
        "keep_theirs",
        9,
      ),
    );
    expect(operationQueueResolveConflict).not.toHaveBeenCalled();
  });
});
