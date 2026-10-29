export type {
  ContextMenuEntry,
  ContextMenuLeafItem,
} from "../model/types/workspace/ExplorerContextMenu";
export {
  canActOnLocalArchiveFile,
  extractArchiveHere,
  extractArchiveTo,
} from "./contextMenu/fileActions";
export {
  compareSeedForPane,
  openCompareWith,
} from "./contextMenu/remoteVerification";
import {
  selectShortcutPreferences,
  useSettingsStore,
} from "@/features/settings";
import { isPackageEntry, tagColors } from "@/features/file-ui";
import {
  detectShortcutPlatform,
  effectiveShortcut,
  formatShortcutLabel,
} from "@/features/shortcuts";
import { isMacPlatform } from "@/shared/platform/tauri";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
  AppWindow,
  Archive,
  ArrowRightLeft,
  Check,
  CheckSquare,
  Clipboard,
  Copy,
  CopyPlus,
  Eye,
  FilePlus,
  Folder,
  FolderInput,
  FolderOpen,
  FolderPlus,
  FolderSearch,
  Hash,
  Info,
  Mail,
  MessageCircle,
  MoreHorizontal,
  Pencil,
  Pin,
  PanelTop,
  Plus,
  Radio,
  RefreshCcw,
  Scissors,
  Share,
  SquareDashed,
  Tag,
  Trash2,
  Undo2,
  X,
} from "lucide-react";
import { memo } from "react";
import type {
  ContextMenuEntry,
  ContextMenuLeafItem,
} from "../model/types/workspace/ExplorerContextMenu";
import { explorerShare, type ShareService } from "../../native";
import { userFacingErrorText } from "@/shared/lib/format";
import { selectedEntriesForPane, useExplorerStore } from "../store";
import { tagsForPath } from "../store/finderTags";
import {
  buildArchiveItems,
  buildFileToolsItems,
} from "./contextMenu/ArchiveToolsItems";
import { openCompareWith } from "./contextMenu/remoteVerification";
import { calculateSelectedFolderSizes } from "./contextMenu/selectionHelpers";
import { useContextMenuState } from "./contextMenu/useContextMenuState";
import { clearSelectionsAcrossPanes } from "./selectionAcrossPanes";
import {
  openInNewTab,
  openWindow,
  quickLookSelected,
  revealSelected,
  runExplorerCommand,
} from "./ExplorerCommands";
import { ExplorerContextMenuView } from "./ExplorerContextMenuView";

/** Finder's colour tags, in its menu order, with their colour indexes. */
const finderColorTags = [
  ["Red", 6],
  ["Orange", 7],
  ["Yellow", 5],
  ["Green", 2],
  ["Blue", 4],
  ["Purple", 3],
  ["Gray", 1],
] as const;

export const ExplorerContextMenu = memo(function ExplorerContextMenu() {
  const shortcutHintsEnabled = useSettingsStore(
    (state) =>
      selectShortcutPreferences(state.settings?.document).shortcutHintsEnabled,
  );
  const shortcutPlatform = detectShortcutPlatform();
  const {
    canCalculateDirectorySizes,
    canCreateFile,
    canCreateFolder,
    canTrashSelection,
    entryId,
    hasClipboard,
    hasPermanentDeleteSelection,
    hasRemoteSelection,
    hasSelection,
    hasEntries,
    inTrash,
    open,
    paneId,
    selectedAcrossPanesCount,
    selectedCount,
    showHidden,
    targetCanOpenWith,
    targetEntry,
    targetPinned,
    virtualListing,
    x,
    y,
  } = useContextMenuState();
  if (!open) return null;

  const selectionDisabledReason = hasSelection
    ? undefined
    : "Select a file or folder first.";
  const createDisabledReason =
    "New items are only available in writable folders.";
  const shortcutFor = (commandId: string) =>
    shortcutHintsEnabled
      ? formatShortcutLabel(
          effectiveShortcut(commandId).primary,
          shortcutPlatform,
        ) || undefined
      : undefined;

  const run = (action: () => void) => {
    useExplorerStore.getState().closeContextMenu();
    action();
  };

  const newItems: ContextMenuLeafItem[] = [
    {
      id: "new-folder",
      icon: <FolderPlus size={17} />,
      label: "New Folder",
      shortcut: shortcutFor("explorer.new_folder"),
      disabled: !canCreateFolder,
      disabledReason: createDisabledReason,
      onRun: () =>
        run(
          () => void useExplorerStore.getState().createItem(paneId, "folder"),
        ),
    },
    {
      id: "new-file",
      icon: <FilePlus size={17} />,
      label: "New File",
      disabled: !canCreateFile,
      disabledReason: createDisabledReason,
      onRun: () =>
        run(() => void useExplorerStore.getState().createItem(paneId, "file")),
    },
    {
      id: "new-folder-with-selection",
      icon: <FolderInput size={17} />,
      label: "New Folder with Selection",
      shortcut: shortcutFor("explorer.new_folder_with_selection"),
      disabled: !hasSelection || !canCreateFolder,
      disabledReason: hasSelection
        ? createDisabledReason
        : selectionDisabledReason,
      onRun: () =>
        run(
          () => void useExplorerStore.getState().newFolderWithSelection(paneId),
        ),
    },
  ];

  const clipboardItems: ContextMenuLeafItem[] = [
    {
      id: "copy",
      icon: <Copy size={17} />,
      label: "Copy",
      shortcut: shortcutFor("explorer.copy"),
      disabled: !hasSelection,
      disabledReason: selectionDisabledReason,
      onRun: () => run(() => useExplorerStore.getState().copySelected(paneId)),
    },
    {
      id: "cut",
      icon: <Scissors size={17} />,
      label: "Cut",
      shortcut: shortcutFor("explorer.cut"),
      disabled: !hasSelection,
      disabledReason: selectionDisabledReason,
      onRun: () => run(() => useExplorerStore.getState().cutSelected(paneId)),
    },
    {
      id: "paste",
      icon: <Clipboard size={17} />,
      label: "Paste",
      shortcut: shortcutFor("explorer.paste"),
      disabled: !hasClipboard,
      disabledReason: hasClipboard ? undefined : "Copy or cut something first.",
      onRun: () =>
        run(() => void useExplorerStore.getState().pasteIntoPane(paneId)),
    },
    ...(targetEntry
      ? [
          {
            id: "copy-name",
            icon: <Copy size={17} />,
            label: selectedCount > 1 ? "Copy Names" : "Copy Name",
            onRun: () =>
              run(() => {
                const pane = useExplorerStore.getState().panes[paneId];
                const selected = new Set(pane?.selectedIds ?? []);
                const names = (pane?.listing?.entries ?? [])
                  .filter((entry) => selected.has(entry.id))
                  .map((entry) => entry.name);
                void writeText(names.join("\n")).then(() =>
                  useExplorerStore
                    .getState()
                    .pushNotification(
                      names.length > 1 ? "Copied names" : "Copied name",
                      "info",
                    ),
                );
              }),
          } satisfies ContextMenuLeafItem,
        ]
      : []),
    {
      id: "select-all",
      icon: <CheckSquare size={17} />,
      label: "Select All",
      shortcut: shortcutHintsEnabled
        ? formatShortcutLabel(
            shortcutPlatform === "macos" ? "Cmd+A" : "Ctrl+A",
            shortcutPlatform,
          )
        : undefined,
      disabled: !hasEntries,
      disabledReason: "This folder is empty.",
      onRun: () =>
        run(() => {
          const store = useExplorerStore.getState();
          store.setSelection(
            paneId,
            (store.panes[paneId]?.listing?.entries ?? []).map(
              (entry) => entry.id,
            ),
          );
        }),
    },
    {
      id: "invert-selection",
      icon: <SquareDashed size={17} />,
      label: "Invert Selection",
      shortcut: shortcutFor("explorer.invert_selection"),
      disabled: !hasEntries,
      disabledReason: "This folder is empty.",
      onRun: () =>
        run(() => useExplorerStore.getState().invertSelection(paneId)),
    },
    {
      id: "deselect",
      icon: <X size={17} />,
      label: "Deselect All",
      disabled: selectedAcrossPanesCount === 0,
      disabledReason: "No selected items.",
      onRun: () => run(clearSelectionsAcrossPanes),
    },
  ];

  const deleteItems: ContextMenuLeafItem[] = [
    ...(!inTrash
      ? [
          {
            id: "trash",
            icon: <Trash2 size={17} />,
            label: "Trash",
            shortcut: shortcutFor("explorer.delete"),
            disabled: !canTrashSelection,
            disabledReason: hasSelection
              ? "Trash is only available for local files and folders."
              : selectionDisabledReason,
            onRun: () =>
              run(
                () =>
                  void useExplorerStore
                    .getState()
                    .deleteSelected(paneId, "trash"),
              ),
          } satisfies ContextMenuLeafItem,
        ]
      : []),
    {
      id: "delete-permanent",
      icon: <X size={17} />,
      label: "Delete Permanently",
      disabled: !hasPermanentDeleteSelection,
      disabledReason: hasPermanentDeleteSelection
        ? undefined
        : selectionDisabledReason,
      onRun: () =>
        run(
          () =>
            void useExplorerStore
              .getState()
              .deleteSelected(paneId, "permanent"),
        ),
    },
  ];

  const archiveToolsContext = {
    hasRemoteSelection,
    hasSelection,
    paneId,
    targetEntry,
    run,
    shortcutFor,
  };
  const archiveItems = buildArchiveItems(archiveToolsContext);
  const fileToolsItems = buildFileToolsItems(archiveToolsContext);
  const renameItems: ContextMenuLeafItem[] = [
    {
      id: "rename-inline",
      icon: <Pencil size={17} />,
      label: "Rename",
      shortcut: shortcutFor("explorer.rename"),
      disabled: !hasSelection,
      disabledReason: selectionDisabledReason,
      onRun: () =>
        run(() => void useExplorerStore.getState().renameSelected(paneId)),
    },
    {
      id: "batch-rename",
      icon: <Pencil size={17} />,
      label: "Batch Rename...",
      disabled: !hasSelection,
      disabledReason: selectionDisabledReason,
      onRun: () =>
        run(() => useExplorerStore.getState().openBatchRenameDialog(paneId)),
    },
  ];

  const moreItems: ContextMenuLeafItem[] = entryId
    ? [
        {
          id: "pin",
          icon: <Pin size={17} />,
          label: targetPinned
            ? "Unpin from Quick access"
            : "Pin to Quick access",
          disabled:
            !targetEntry ||
            targetEntry.isDeleted ||
            targetEntry.kind !== "folder",
          disabledReason: "Only folders can be pinned.",
          onRun: () =>
            run(
              () =>
                targetEntry &&
                useExplorerStore.getState().togglePinnedPath(targetEntry.path),
            ),
        },
        {
          id: "folder-sizes",
          icon: <Folder size={17} />,
          label: "Calculate Folder Sizes",
          disabled: !canCalculateDirectorySizes,
          disabledReason: "Select one or more folders.",
          onRun: () => run(() => calculateSelectedFolderSizes(paneId)),
        },
      ]
    : [
        {
          id: "hidden-files",
          icon: <Eye size={17} />,
          label: showHidden ? "Hide Hidden Files" : "Show Hidden Files",
          shortcut: shortcutFor("explorer.toggle_hidden"),
          onRun: () =>
            run(() => void useExplorerStore.getState().toggleHidden(paneId)),
        },
      ];

  const localTarget = Boolean(
    targetEntry &&
    !targetEntry.isDeleted &&
    targetEntry.location.kind === "local",
  );
  const openEntries: ContextMenuEntry[] =
    targetEntry && !inTrash
      ? [
          {
            id: "open",
            icon: <FolderOpen size={17} />,
            label: "Open",
            onRun: () =>
              run(
                () =>
                  void useExplorerStore
                    .getState()
                    .openEntry(paneId, targetEntry),
              ),
          },
          ...(isPackageEntry(targetEntry) && localTarget
            ? [
                {
                  id: "show-package-contents",
                  icon: <FolderOpen size={17} />,
                  label: "Show Package Contents",
                  onRun: () =>
                    run(
                      () =>
                        void useExplorerStore
                          .getState()
                          .navigatePane(paneId, targetEntry.path),
                    ),
                } satisfies ContextMenuLeafItem,
              ]
            : []),
          ...(targetEntry.kind === "folder"
            ? [
                {
                  id: "open-in-new-tab",
                  icon: <PanelTop size={17} />,
                  label: "Open in New Tab",
                  shortcut: shortcutFor("explorer.open_in_new_tab"),
                  onRun: () => run(() => openInNewTab(targetEntry.path)),
                } satisfies ContextMenuLeafItem,
                ...(localTarget
                  ? [
                      {
                        id: "open-in-new-window",
                        icon: <AppWindow size={17} />,
                        label: "Open in New Window",
                        onRun: () => run(() => openWindow(targetEntry.path)),
                      } satisfies ContextMenuLeafItem,
                    ]
                  : []),
              ]
            : []),
          {
            id: "open-with",
            icon: <AppWindow size={17} />,
            label: "Open With...",
            disabled: !targetCanOpenWith,
            disabledReason: "Open With is available for files.",
            onRun: () =>
              run(
                () => void useExplorerStore.getState().openWithSelected(paneId),
              ),
          },
          ...(targetEntry.kind !== "folder" && targetEntry.kind !== "symlink"
            ? [
                {
                  id: "quick-look",
                  icon: <Eye size={17} />,
                  label: "Quick Look",
                  shortcut: shortcutHintsEnabled ? "Space" : undefined,
                  onRun: () => run(() => quickLookSelected(paneId)),
                } satisfies ContextMenuLeafItem,
              ]
            : []),
          {
            id: "get-info",
            icon: <Info size={17} />,
            label: "Get Info",
            shortcut: shortcutFor("explorer.get_info"),
            onRun: () =>
              run(() => runExplorerCommand("explorer.get_info", paneId)),
          },
          ...(virtualListing && localTarget
            ? [
                {
                  id: "show-in-folder",
                  icon: <FolderSearch size={17} />,
                  label: "Show in Enclosing Folder",
                  onRun: () =>
                    run(
                      () =>
                        void useExplorerStore
                          .getState()
                          .showInEnclosingFolder(paneId, targetEntry),
                    ),
                } satisfies ContextMenuLeafItem,
              ]
            : []),
          ...(localTarget
            ? [
                {
                  id: "reveal",
                  icon: <FolderSearch size={17} />,
                  label: isMacPlatform()
                    ? "Reveal in Finder"
                    : "Show in File Manager",
                  shortcut: shortcutFor("explorer.reveal"),
                  onRun: () => run(() => revealSelected(paneId)),
                } satisfies ContextMenuLeafItem,
              ]
            : []),
        ]
      : [];
  const trashEntries: ContextMenuEntry[] = inTrash
    ? [
        ...(targetEntry
          ? [
              {
                id: "put-back",
                icon: <Undo2 size={17} />,
                label: "Put Back",
                shortcut: shortcutFor("explorer.delete"),
                disabled: !hasPermanentDeleteSelection,
                disabledReason: selectionDisabledReason,
                onRun: () =>
                  run(
                    () =>
                      void useExplorerStore.getState().putBackSelected(paneId),
                  ),
              } satisfies ContextMenuLeafItem,
            ]
          : []),
        {
          id: "empty-trash",
          icon: <Trash2 size={17} />,
          label: "Empty Trash...",
          shortcut: shortcutFor("explorer.empty_trash"),
          disabled: !hasEntries,
          disabledReason: "The Trash is empty.",
          onRun: () =>
            run(() => void useExplorerStore.getState().emptyTrash(paneId)),
        },
      ]
    : [];
  const duplicateEntries: ContextMenuEntry[] =
    targetEntry && localTarget && !virtualListing
      ? [
          {
            id: "duplicate",
            icon: <CopyPlus size={17} />,
            label: "Duplicate",
            shortcut: shortcutFor("explorer.duplicate"),
            onRun: () =>
              run(
                () =>
                  void useExplorerStore.getState().duplicateSelected(paneId),
              ),
          },
        ]
      : [];

  const tagEntries: ContextMenuEntry[] =
    targetEntry && localTarget && isMacPlatform()
      ? [
          {
            id: "tags",
            icon: <Tag size={17} />,
            label: "Tags",
            items: [
              ...finderColorTags.map(([name, color]) => ({
                id: `tag-${name}`,
                icon: (
                  <span
                    aria-hidden="true"
                    className="size-3 rounded-full"
                    style={{
                      backgroundColor: tagColors[color],
                      boxShadow: tagsForPath(targetEntry.path).some(
                        (tag) => tag.name === name,
                      )
                        ? "0 0 0 2px var(--color-charcoal-bg), 0 0 0 3.5px currentColor"
                        : undefined,
                    }}
                  />
                ),
                label: name,
                onRun: () =>
                  run(
                    () =>
                      void useExplorerStore.getState().toggleTag(paneId, name),
                  ),
              })),
              ...tagsForPath(targetEntry.path)
                .filter(
                  (tag) =>
                    !finderColorTags.some(
                      ([colorName]) => colorName === tag.name,
                    ),
                )
                .map((tag) => ({
                  id: `tag-custom-${tag.name}`,
                  icon: <Check size={15} />,
                  label: tag.name,
                  onRun: () =>
                    run(
                      () =>
                        void useExplorerStore
                          .getState()
                          .toggleTag(paneId, tag.name),
                    ),
                })),
              {
                id: "tag-new",
                icon: <Plus size={15} />,
                label: "New Tag...",
                onRun: () =>
                  run(() =>
                    useExplorerStore.setState({
                      dialog: { kind: "newTag", paneId },
                    }),
                  ),
              },
            ],
          },
        ]
      : [];

  const share = (service: ShareService) =>
    run(() => {
      const store = useExplorerStore.getState();
      const paths = selectedEntriesForPane(store.panes[paneId])
        .filter((entry) => entry.location.kind === "local" && !entry.isDeleted)
        .map((entry) => entry.path);
      if (paths.length === 0) return;
      explorerShare(paths, service).catch((error: unknown) =>
        store.pushNotification(
          `Could not share: ${userFacingErrorText(error)}`,
          "error",
          4500,
        ),
      );
    });
  const shareEntries: ContextMenuEntry[] =
    targetEntry && localTarget && isMacPlatform()
      ? [
          {
            id: "share",
            icon: <Share size={17} />,
            label: "Share",
            items: [
              {
                id: "share-airdrop",
                icon: <Radio size={17} />,
                label: "AirDrop",
                onRun: () => share("airdrop"),
              },
              {
                id: "share-mail",
                icon: <Mail size={17} />,
                label: "Mail",
                onRun: () => share("mail"),
              },
              {
                id: "share-messages",
                icon: <MessageCircle size={17} />,
                label: "Messages",
                onRun: () => share("messages"),
              },
            ],
          },
        ]
      : [];

  const menuEntries: ContextMenuEntry[] = [
    ...openEntries,
    ...trashEntries,
    // New and Refresh act on the folder, so items keep a shorter menu.
    ...(targetEntry
      ? []
      : [
          {
            id: "new",
            icon: <FolderPlus size={17} />,
            label: "New",
            items: newItems,
          } satisfies ContextMenuEntry,
        ]),
    {
      id: "clipboard",
      icon: <Clipboard size={17} />,
      label: "Clipboard",
      items: clipboardItems,
    },
    {
      id: "rename",
      icon: <Pencil size={17} />,
      label: "Rename",
      disabled: !hasSelection,
      disabledReason: selectionDisabledReason,
      items: renameItems,
    },
    ...duplicateEntries,
    ...tagEntries,
    ...shareEntries,
    {
      id: "delete",
      icon: <Trash2 size={17} />,
      label: "Delete",
      items: deleteItems,
    },
    {
      id: "archive",
      icon: <Archive size={17} />,
      label: "Archive",
      items: archiveItems,
    },
    {
      id: "file-tools",
      icon: <Hash size={17} />,
      label: "File Tools",
      items: [
        {
          id: "compare-with",
          icon: <ArrowRightLeft size={17} />,
          label: "Compare With...",
          disabled: !hasSelection || hasRemoteSelection,
          disabledReason: hasRemoteSelection
            ? "Compare is available for local files and folders."
            : selectionDisabledReason,
          onRun: () => run(() => openCompareWith(paneId)),
        },
        ...fileToolsItems,
      ],
    },
    {
      id: "more",
      icon: <MoreHorizontal size={17} />,
      label: "More",
      items: moreItems,
    },
    ...(targetEntry
      ? []
      : [
          {
            id: "refresh",
            icon: <RefreshCcw size={17} />,
            label: "Refresh",
            shortcut: shortcutFor("explorer.refresh"),
            onRun: () =>
              run(() => void useExplorerStore.getState().refreshPane(paneId)),
          } satisfies ContextMenuEntry,
        ]),
  ];

  return (
    <>
      <ExplorerContextMenuView
        open={open}
        x={x}
        y={y}
        menuEntries={menuEntries}
        onClose={() => useExplorerStore.getState().closeContextMenu()}
      />
    </>
  );
});
