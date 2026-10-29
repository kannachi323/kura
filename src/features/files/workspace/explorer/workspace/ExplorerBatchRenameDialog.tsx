import { ExplorerDeleteDialogView } from "./ExplorerDeleteDialogView";
import { useExplorerStore } from "../store";
import { BatchRenameDialogView } from "./BatchRenameDialogView";
import { NewTagDialogView } from "./NewTagDialogView";

export function ExplorerDialog() {
  const dialog = useExplorerStore((state) => state.dialog);
  if (!dialog) return null;
  if (dialog.kind === "newTag") {
    return (
      <NewTagDialogView
        onClose={() => useExplorerStore.getState().closeDialog()}
        onApply={(name) =>
          useExplorerStore.getState().toggleTag(dialog.paneId, name, true)
        }
      />
    );
  }
  if (dialog.kind === "batchRename") {
    return (
      <BatchRenameDialogView
        dialog={dialog}
        onClose={() => useExplorerStore.getState().closeDialog()}
        onApply={async (items) => {
          const store = useExplorerStore.getState();
          store.setBatchRenameItems(dialog.paneId, items);
          await store.confirmDialog();
        }}
      />
    );
  }
  return (
    <ExplorerDeleteDialogView
      paths={dialog.paths}
      onClose={() => useExplorerStore.getState().closeDialog()}
      onConfirm={() => useExplorerStore.getState().confirmDialog()}
    />
  );
}
