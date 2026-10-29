import {
  selectGeneralPreferences,
  useSettingsStore,
} from "@/features/settings";
import {
  explorerQueueCreateItem,
  explorerListDirectory,
  explorerPathIsDirectory,
  explorerPutBackItems,
  explorerSetTag,
  explorerQueueDeleteItems,
  explorerQueuePasteItems,
  explorerQueueRenameItem,
  explorerQueueRenameItems,
} from "../../../native";
import { userFacingErrorText } from "@/shared/lib/format";
import type {
  ExplorerInlineEditState,
  ExplorerStore,
} from "../../model/interfaces/store/types";
import type { ExplorerGet, ExplorerSet } from "../../model/types/store/types";
import * as H from "../helpers";
import { explorerRuntime } from "../runtime";
import { copyName, uniqueName } from "../../utils/copyName";
import { forgetTags, tagsForPath } from "../finderTags";

export function createMutationsActions(
  set: ExplorerSet,
  get: ExplorerGet,
): Partial<ExplorerStore> {
  return {
    canCreateItem: (paneId, kind) =>
      H.canCreateItemInPane(get().panes[paneId], kind, get().inlineEdit),

    createItem: async (paneId, kind, name) => {
      const pane = get().panes[paneId];
      const directory = pane?.listing?.path;
      if (!directory) return;
      const defaultName =
        kind === "folder" ? "Untitled Folder" : "Untitled File";
      if (name == null) {
        if (!H.canCreateItemInPane(pane, kind, get().inlineEdit)) return;
        const inlineEdit: ExplorerInlineEditState = {
          paneId,
          kind: "create",
          itemKind: kind,
          entryId: null,
          originalName: "",
          value: defaultName,
          lockedExtension: "",
          error: null,
        };
        set({ inlineEdit: H.withInlineEditValidation(inlineEdit, pane) });
        return;
      }
      const inlineEdit = get().inlineEdit;
      const submittingCurrentCreate =
        inlineEdit?.kind === "create" &&
        inlineEdit.paneId === paneId &&
        inlineEdit.itemKind === kind;
      if (
        !H.canCreateItemInPane(
          pane,
          kind,
          submittingCurrentCreate ? null : inlineEdit,
        )
      )
        return;
      const requestedName = name;
      if (!requestedName) return;
      try {
        set({ operationError: null });
        await explorerQueueCreateItem({ directory, name: requestedName, kind });
        H.refreshFileOperationState();
        get().pushNotification(
          `Queued ${kind === "folder" ? "folder" : "file"} creation`,
          "success",
        );
        H.queuePaneRefresh(paneId, directory);
      } catch (error) {
        set({ operationError: userFacingErrorText(error) });
      }
    },

    renameSelected: async (paneId, name) => {
      const pane = get().panes[paneId];
      const entries = H.selectedEntriesForPane(pane);
      const entry = entries[0] ?? null;
      if (name == null) {
        const renameItems = H.selectedBatchRenameItemsAcrossPanes(
          get().panes,
          paneId,
        );
        if (renameItems.length === 0) return;
        const targetItem = renameItems[0];
        const targetPaneId = targetItem?.paneId ?? paneId;
        const targetPane = get().panes[targetPaneId] ?? pane;
        const targetEntry =
          targetPane?.listing?.entries.find(
            (candidate) => candidate.id === targetItem?.entryId,
          ) ?? entry;
        const [value, lockedExtension] = H.splitRenameParts(targetEntry);
        const inlineEdit: ExplorerInlineEditState = {
          paneId: targetPaneId,
          kind: "rename",
          itemKind: targetEntry.kind === "folder" ? "folder" : "file",
          entryId: targetEntry.id,
          originalName: targetEntry.name,
          value,
          lockedExtension,
          batchItems: renameItems.length > 1 ? renameItems : undefined,
          error: null,
        };
        set({ inlineEdit: H.withInlineEditValidation(inlineEdit, targetPane) });
        return;
      }
      if (!entry) return;
      const requestedName = name;
      if (!requestedName || requestedName === entry.name) return;
      try {
        set({ operationError: null });
        await explorerQueueRenameItem({
          path: entry.path,
          newName: requestedName,
          sourceIsDirectory: entry.kind === "folder",
        });
        H.refreshFileOperationState();
        get().pushNotification(`Queued rename to ${requestedName}`, "success");
        get().clearSelection(paneId);
        H.queuePaneRefresh(paneId, pane?.listing?.path ?? entry.path);
      } catch (error) {
        set({ operationError: userFacingErrorText(error) });
      }
    },

    openBatchRenameDialog: (paneId) => {
      const items = H.selectedBatchRenameItemsAcrossPanes(get().panes, paneId);
      if (items.length === 0) return;
      set({
        inlineEdit: null,
        dialog: {
          kind: "batchRename",
          paneId,
          items: H.validateBatchRenameItems(items),
        },
      });
    },

    deleteSelected: async (paneId, mode) => {
      const pane = get().panes[paneId];
      const permanent =
        H.deleteModeForPaneSelection(pane, mode) === "permanent";
      const paths = H.selectedDeletePathsForPane(pane, permanent);
      if (paths.length === 0) return;
      const isTrashPane = pane?.listing?.path === "kura://trash";
      const shouldConfirm =
        permanent &&
        !isTrashPane &&
        selectGeneralPreferences(useSettingsStore.getState().settings?.document)
          .confirmDestructiveActions;
      if (!shouldConfirm) {
        try {
          set({ operationError: null });
          const directory = pane?.listing?.path;
          await explorerQueueDeleteItems({ paths, permanent });
          H.refreshFileOperationState();
          get().pushNotification(
            H.deleteQueuedMessage(paths.length, permanent),
            "success",
          );
          get().clearSelection(paneId);
          if (directory) H.queuePaneRefresh(paneId, directory);
        } catch (error) {
          set({ operationError: userFacingErrorText(error) });
        }
        return;
      }
      set({ dialog: { kind: "delete", paneId, paths, permanent } });
    },

    duplicateSelected: async (paneId) => {
      const pane = get().panes[paneId];
      const directory = pane?.listing?.path;
      if (!pane?.listing || !directory || directory.startsWith("kura://"))
        return;
      const entries = H.selectedEntriesForPane(pane).filter(
        (entry) => !entry.isDeleted && entry.location.kind === "local",
      );
      if (entries.length === 0) return;
      const taken = new Set([
        ...pane.listing.entries.map((entry) => entry.name.toLowerCase()),
        ...reservedNames(directory),
      ]);
      try {
        set({ operationError: null });
        let lastName = "";
        for (const entry of entries) {
          const targetName = copyName(
            entry.name,
            entry.kind === "folder",
            taken,
          );
          taken.add(targetName.toLowerCase());
          reserveName(directory, targetName);
          lastName = targetName;
          await explorerQueuePasteItems({
            sources: [H.pasteItemForEntry(entry)],
            destinationDirectory: directory,
            operation: "copy",
            targetName,
          });
        }
        H.refreshFileOperationState();
        explorerRuntime.pendingSelections.set(paneId, {
          path: joinPath(directory, lastName),
          expiresAtMs: Date.now() + 10_000,
        });
        H.queuePaneRefresh(paneId, directory);
      } catch (error) {
        set({ operationError: userFacingErrorText(error) });
      }
    },

    newFolderWithSelection: async (paneId) => {
      const pane = get().panes[paneId];
      const directory = pane?.listing?.path;
      if (!pane?.listing || !directory || directory.startsWith("kura://"))
        return;
      const items = H.selectedPasteItemsForPane(pane);
      if (items.length === 0) return;
      const taken = new Set(
        pane.listing.entries.map((entry) => entry.name.toLowerCase()),
      );
      const name = uniqueName("New Folder With Items", taken);
      const folder = joinPath(directory, name);
      try {
        set({ operationError: null });
        await explorerQueueCreateItem({ directory, name, kind: "folder" });
        // The move needs the folder to exist; creates run in milliseconds.
        const deadline = Date.now() + 5_000;
        while (!(await explorerPathIsDirectory(folder))) {
          if (Date.now() > deadline)
            throw new Error(`Could not create “${name}”.`);
          await new Promise((resolve) => setTimeout(resolve, 50));
        }
        await explorerQueuePasteItems({
          sources: items,
          destinationDirectory: folder,
          operation: "move",
        });
        H.refreshFileOperationState();
        explorerRuntime.pendingSelections.set(paneId, {
          path: folder,
          expiresAtMs: Date.now() + 10_000,
        });
        H.queuePaneRefresh(paneId, directory);
      } catch (error) {
        set({ operationError: userFacingErrorText(error) });
      }
    },

    showInEnclosingFolder: async (paneId, entry) => {
      const separator = Math.max(
        entry.path.lastIndexOf("/"),
        entry.path.lastIndexOf("\\"),
      );
      const parent = entry.path.slice(0, separator) || "/";
      explorerRuntime.pendingSelections.set(paneId, {
        path: entry.path,
        expiresAtMs: Date.now() + 10_000,
      });
      await get().navigatePane(paneId, parent);
    },

    toggleTag: async (paneId, name) => {
      const pane = get().panes[paneId];
      const entries = H.selectedEntriesForPane(pane).filter(
        (entry) => !entry.isDeleted && entry.location.kind === "local",
      );
      if (entries.length === 0) return;
      // Like Finder: if every selected item has the tag, remove it.
      const enabled = !entries.every((entry) =>
        tagsForPath(entry.path).some((tag) => tag.name === name),
      );
      const paths = entries.map((entry) => entry.path);
      try {
        set({ operationError: null });
        await explorerSetTag(paths, name, enabled);
        forgetTags(paths);
      } catch (error) {
        set({ operationError: userFacingErrorText(error) });
      }
    },

    putBackSelected: async (paneId) => {
      const pane = get().panes[paneId];
      const paths = H.selectedDeletePathsForPane(pane, true);
      if (paths.length === 0) return;
      try {
        set({ operationError: null });
        await explorerPutBackItems(paths);
        get().pushNotification(
          `Put back ${H.itemCountLabel(paths.length)}`,
          "success",
        );
        get().clearSelection(paneId);
        H.queuePaneRefresh(paneId, "kura://trash");
      } catch (error) {
        set({ operationError: userFacingErrorText(error) });
      }
    },

    emptyTrash: async (paneId) => {
      const listing = get().panes[paneId]?.listing;
      let entries = listing?.path === "kura://trash" ? listing.entries : null;
      try {
        entries ??= (await explorerListDirectory({ path: "kura://trash" }))
          .entries;
      } catch (error) {
        set({ operationError: userFacingErrorText(error) });
        return;
      }
      const paths = entries.map((entry) => entry.path);
      if (paths.length === 0) {
        get().pushNotification("The Trash is empty.", "info", 3000);
        return;
      }
      // Always confirmed, as in Finder: this can't be undone.
      set({ dialog: { kind: "delete", paneId, paths, permanent: true } });
    },

    downloadSelected: async (paneId) => {
      const pane = get().panes[paneId];
      const items = H.selectedRemotePasteItemsForPane(pane);
      if (items.length === 0) return;
      try {
        const downloadsDirectory = await H.downloadDestinationDirectory();
        if (!downloadsDirectory) return;
        set({ operationError: null });
        await explorerQueuePasteItems({
          sources: items,
          destinationDirectory: downloadsDirectory,
          operation: "copy",
        });
        H.refreshFileOperationState();
        get().pushNotification(
          `Queued download for ${H.itemCountLabel(items.length)}`,
          "success",
        );
        if (
          pane?.listing &&
          H.samePath(pane.listing.path, downloadsDirectory)
        ) {
          H.queuePaneRefresh(paneId, downloadsDirectory);
        }
      } catch (error) {
        set({
          operationError: `Download failed: ${userFacingErrorText(error)}`,
        });
      }
    },

    confirmDialog: async () => {
      const dialog = get().dialog;
      if (!dialog) return;
      if (dialog.kind === "delete") {
        set({ dialog: null });
        try {
          set({ operationError: null });
          const directory = get().panes[dialog.paneId]?.listing?.path;
          await explorerQueueDeleteItems({
            paths: dialog.paths,
            permanent: dialog.permanent,
          });
          H.refreshFileOperationState();
          get().pushNotification(
            H.deleteQueuedMessage(dialog.paths.length, dialog.permanent),
            "success",
          );
          get().clearSelection(dialog.paneId);
          if (directory) H.queuePaneRefresh(dialog.paneId, directory);
        } catch (error) {
          set({ operationError: userFacingErrorText(error) });
        }
        return;
      }

      const validatedItems = H.validateBatchRenameItems(dialog.items);
      const items = validatedItems
        .map((item) => ({
          item,
          effectiveName: `${item.value.trim()}${item.lockedExtension}`,
        }))
        .filter(
          ({ item, effectiveName }) =>
            !item.error && effectiveName !== item.originalName,
        )
        .map(({ item, effectiveName }) => ({
          path: item.path,
          newName: effectiveName,
          sourceIsDirectory: item.isDirectory,
        }));
      if (items.length === 0) {
        const hasErrors = validatedItems.some((item) => item.error);
        set(
          hasErrors
            ? { dialog: { ...dialog, items: validatedItems } }
            : { dialog: null },
        );
        return;
      }
      set({ dialog: null });
      try {
        set({ operationError: null });
        await explorerQueueRenameItems({ items });
        H.refreshFileOperationState();
        get().pushNotification(
          `Queued rename for ${H.itemCountLabel(items.length)}`,
          "success",
        );
        H.refreshAndClearRenamePanes(validatedItems);
      } catch (error) {
        set({
          operationError: userFacingErrorText(error),
          dialog: { ...dialog, items: validatedItems },
        });
      }
    },

    setInlineEditValue: (value) =>
      set((state) => {
        if (!state.inlineEdit) return state;
        const draft = { ...state.inlineEdit, value };
        return {
          inlineEdit: H.withInlineEditValidation(
            draft,
            state.panes[draft.paneId],
          ),
        };
      }),

    setBatchRenameValue: (paneId, entryId, value) =>
      set((state) => {
        const dialog = state.dialog;
        if (dialog?.kind !== "batchRename") return state;
        const items = H.validateBatchRenameItems(
          dialog.items.map((item) =>
            item.paneId === paneId && item.entryId === entryId
              ? { ...item, value }
              : item,
          ),
        );
        return { dialog: { ...dialog, items } };
      }),

    setBatchRenameItems: (paneId, items) =>
      set((state) => {
        const dialog = state.dialog;
        if (dialog?.kind !== "batchRename" || dialog.paneId !== paneId)
          return state;
        return {
          dialog: { ...dialog, items: H.validateBatchRenameItems(items) },
        };
      }),

    commitInlineEdit: async () => {
      const edit = get().inlineEdit;
      if (!edit) return;
      const pane = get().panes[edit.paneId];
      const validated = H.withInlineEditValidation(edit, pane);
      const batchRename =
        validated.kind === "rename" &&
        validated.batchItems &&
        validated.batchItems.length > 1;
      if (validated.error && !batchRename) {
        set({ inlineEdit: validated });
        get().pushNotification(validated.error, "error", 4500);
        return;
      }
      const effectiveName = `${validated.value.trim()}${validated.lockedExtension}`;
      if (
        validated.kind === "rename" &&
        !batchRename &&
        effectiveName === validated.originalName
      ) {
        set({ inlineEdit: null });
        return;
      }
      if (validated.kind === "create") {
        await get().createItem(
          validated.paneId,
          validated.itemKind,
          effectiveName,
        );
      } else if (validated.batchItems && validated.batchItems.length > 1) {
        set({
          inlineEdit: null,
          dialog: {
            kind: "batchRename",
            paneId: validated.paneId,
            items: H.validateBatchRenameItems(validated.batchItems),
          },
        });
        return;
      } else {
        await get().renameSelected(validated.paneId, effectiveName);
      }
      const operationError = get().operationError;
      if (operationError && get().inlineEdit === edit) {
        set({ inlineEdit: { ...validated, error: operationError } });
      } else if (get().inlineEdit === edit) {
        set({ inlineEdit: null });
      }
      const directory = pane?.listing?.path;
      if (!operationError && directory) {
        // Like Finder, the new or renamed item ends up selected.
        const separator =
          directory.includes("\\") && !directory.includes("/") ? "\\" : "/";
        explorerRuntime.pendingSelections.set(validated.paneId, {
          path: directory.endsWith(separator)
            ? `${directory}${effectiveName}`
            : `${directory}${separator}${effectiveName}`,
          expiresAtMs: Date.now() + 10_000,
        });
      }
    },

    cancelInlineEdit: () =>
      set((state) => (state.inlineEdit ? { inlineEdit: null } : state)),

    closeDialog: () =>
      set((state) => {
        const dialog = state.dialog;
        if (!dialog) return state;
        if (dialog.kind !== "batchRename") return { dialog: null };
        return {
          dialog: null,
          inlineEdit: H.inlineEditFromBatchRenameDialog(dialog),
        };
      }),
  };
}

function joinPath(directory: string, name: string): string {
  const separator =
    directory.includes("\\") && !directory.includes("/") ? "\\" : "/";
  return directory.endsWith(separator)
    ? `${directory}${name}`
    : `${directory}${separator}${name}`;
}

/**
 * Names chosen for copies that may not be in the listing yet. Without this a
 * second Cmd+D before the refresh picks the same "copy" name, and the two
 * queued copies race past the conflict check.
 */
const reservedCopyNames = new Map<string, Map<string, number>>();

function reserveName(directory: string, name: string): void {
  const names = reservedCopyNames.get(directory) ?? new Map<string, number>();
  names.set(name.toLowerCase(), Date.now() + 10_000);
  reservedCopyNames.set(directory, names);
}

function reservedNames(directory: string): string[] {
  const names = reservedCopyNames.get(directory);
  if (!names) return [];
  const now = Date.now();
  for (const [name, expiresAtMs] of names)
    if (expiresAtMs < now) names.delete(name);
  return [...names.keys()];
}
