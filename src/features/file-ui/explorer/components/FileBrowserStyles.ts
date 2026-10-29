import { cn } from "@/shared/ui";
export const fileBrowserStyles = {
  browser: [
    "relative grid min-h-0 min-w-0 grid-rows-[minmax(0,1fr)_36px] overflow-hidden",
    "bg-charcoal-sidebar outline outline-0 outline-offset-[-2px]",
  ].join(" "),
  browserLoading: "bg-charcoal-sidebar",
  tableSkeleton: "min-h-0 min-w-[720px] overflow-hidden",
  tableSkeletonLine:
    "grid grid-cols-[minmax(240px,1fr)_220px_128px_128px] items-center gap-4 px-3.5",
  tableSkeletonHeader: "h-8 bg-charcoal-card",
  tableSkeletonRow: "h-[30px] [[data-compact-mode=true]_&]:h-[26px]",
  skeletonCell: "kura-skeleton rounded-md bg-charcoal-card",
  tableSkeletonHeaderCell: "h-[13px]",
  tableSkeletonCell: "h-3 first:h-4",
  gridSkeleton: cn(
    "grid min-h-0 min-w-0 content-start gap-[18px] overflow-hidden p-1",
    "[grid-template-columns:repeat(auto-fill,120px)]",
    "[[data-compact-mode=true]_&]:gap-2.5 [[data-compact-mode=true]_&]:p-0.5",
    "[[data-compact-mode=true]_&]:[grid-template-columns:repeat(auto-fill,108px)]",
  ),
  gridSkeletonCell: "h-[136px] [[data-compact-mode=true]_&]:h-[120px]",
  // A container, so the compact table follows the pane's width (split panes,
  // sidebar and inspector open), not the window's.
  tableWrap:
    "@container grid min-h-0 min-w-0 grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-charcoal-sidebar",
  tableHeaderWrap: "min-w-0 overflow-hidden",
  tableResetButton: "absolute right-2 top-1.5 z-[4] @max-[640px]:hidden",
  marquee:
    "pointer-events-none absolute z-[5] rounded-[3px] border border-accent-blue/80 bg-accent-blue/15",
  tableScroll:
    "kura-transient-scrollbar relative kura-file-browser-scrollbar min-h-0 min-w-0 overflow-auto [contain:layout_paint] [overscroll-behavior:contain]",
  table: cn(
    "w-full min-w-[640px] table-fixed border-collapse",
    // The column widths are inline styles; a narrow pane overrides them.
    "@max-[640px]:w-full! @max-[640px]:min-w-0!",
    "@max-[640px]:[&_col:first-child]:w-[58%]! @max-[640px]:[&_col:nth-child(2)]:w-[42%]!",
    "@max-[640px]:[&_col:nth-child(n+3)]:w-0!",
    // Very narrow panes show names only.
    "@max-[340px]:[&_col:first-child]:w-full! @max-[340px]:[&_col:nth-child(2)]:w-0!",
    "@max-[340px]:[&_td:nth-child(2)]:hidden @max-[340px]:[&_th:nth-child(2)]:hidden",
    "@max-[640px]:[&_td:first-child]:w-[58%]",
    "@max-[640px]:[&_td:nth-child(2)]:w-[42%]",
    "@max-[640px]:[&_td:nth-child(n+3)]:hidden",
    "@max-[640px]:[&_th:first-child]:w-[58%]",
    "@max-[640px]:[&_th:nth-child(2)]:w-[42%]",
    "@max-[640px]:[&_th:nth-child(n+3)]:hidden",
  ),
  tableHeadCell: "group/header relative overflow-hidden whitespace-nowrap",
  tableHeadFiller: "bg-transparent p-0 shadow-none @max-[640px]:hidden",
  tableSort: cn(
    "-mx-2 flex h-8 w-[calc(100%+1rem)] min-w-0 items-center justify-start gap-1.5 text-[12px]",
    "overflow-hidden rounded-none border-0 bg-transparent px-2 py-0 text-left",
    "font-medium text-cream-muted shadow-none hover:bg-transparent",
    "hover:text-cream",
  ),
  tableSortLabel: "min-w-0 overflow-hidden text-ellipsis",
  tableSortActive: "text-cream",
  tableSortIndicator:
    "inline-flex size-[13px] flex-none items-center justify-center text-cream-muted",
  tableResizeHandle: cn(
    "absolute -right-1 top-0 z-[2] h-full w-[8px]",
    "cursor-col-resize after:absolute after:inset-x-0 after:bottom-[8px] after:mx-auto",
    "after:top-[8px] after:w-px after:bg-transparent",
    "after:content-[''] group-hover/header:after:bg-charcoal-border",
    "@max-[640px]:hidden",
  ),
  tableResizeHandleActive: "after:!bg-charcoal-active",
  tableRow: cn(
    "group/file-row cursor-default select-none border-b-0 text-[13px] outline-none",
    "hover:bg-charcoal-card data-[state=selected]:bg-selection",
    "data-[state=selected]:text-cream focus-visible:relative",
    "focus-visible:z-[2] focus-visible:ring-2 focus-visible:ring-inset",
    "focus-visible:ring-charcoal-active/50",
  ),
  tableRowDragging: "opacity-50",
  tableRowCut: "opacity-55",
  tableRowDeleted: "text-cream-muted opacity-55",
  tableRowInlineEditing: "relative z-[3]",
  tableCell:
    "cursor-default select-none overflow-hidden text-ellipsis whitespace-nowrap text-cream-muted",
  tableDateCell: "tabular-nums",
  tableNumericCell: "text-left tabular-nums",
  tableNumericHeader: "[&_button]:justify-start",
  directorySizeDots: "inline-flex h-5 w-8 items-center gap-1 align-middle",
  directorySizeDot:
    "size-1.5 rounded-full bg-cream-muted opacity-85 motion-safe:animate-bounce",
  tableFillerCell: "p-0 @max-[640px]:hidden",
  tableNameCell: "cursor-default select-none overflow-hidden",
  tableNameCellEditing: "overflow-visible",
  tableNameText:
    "min-w-0 cursor-default select-none overflow-hidden text-ellipsis whitespace-nowrap",
  tableIconSlot: "grid size-5 flex-none place-items-center",
  entryIcon: "block shrink-0",
  // Untoned icons brighten with their row; toned icons keep their color.
  entryIconMuted:
    "text-cream-muted group-data-[state=selected]/file-row:text-cream group-aria-pressed/file-row:text-cream group-hover/file-row:text-cream",
  gridScroll:
    "kura-transient-scrollbar kura-file-browser-scrollbar min-h-0 min-w-0 overflow-auto [contain:layout_paint] [overscroll-behavior:contain]",
  gridSizer: "relative min-w-0",
  grid: cn(
    "absolute left-[3px] right-[3px] grid content-start justify-start gap-3",
    "[[data-compact-mode=true]_&]:left-0.5",
    "[[data-compact-mode=true]_&]:right-0.5",
    "[[data-compact-mode=true]_&]:gap-2",
  ),
  gridItem: cn(
    "group/file-row relative grid min-w-0 content-start",
    "cursor-default justify-items-center gap-2 rounded-md border",
    "border-transparent bg-transparent px-1 py-2.5 text-cream-muted",
    "outline-none hover:bg-charcoal-card hover:text-cream",
    "aria-pressed:bg-selection aria-pressed:text-cream",
    "focus-visible:border-charcoal-active focus-visible:ring-1",
    "focus-visible:ring-charcoal-active",
    "[[data-compact-mode=true]_&]:gap-1.5 [[data-compact-mode=true]_&]:py-2",
  ),
  gridItemSelected:
    "selected bg-selection text-cream hover:bg-selection hover:text-cream",
  gridItemDragging: "opacity-50",
  gridItemCut: "opacity-55",
  gridItemDeleted:
    "deleted text-cream-muted [&>span:not(.inline-name-editor)]:opacity-[0.86]",
  gridItemInlineEdit: "relative z-[2]",
  gridThumb:
    "grid aspect-[5/3] w-full place-items-center overflow-hidden rounded-md border border-charcoal-border bg-charcoal-card",
  gridThumbImage: "h-full w-full object-cover",
  gridThumbIcon: "grid aspect-[5/3] w-full place-items-center",
  gridNameText:
    "line-clamp-2 max-w-full overflow-hidden break-words text-center text-[12px] leading-[1.3] text-cream/90",
  inlineEditor:
    "inline-name-editor relative inline-flex min-w-0 max-w-full items-center",
  inlineEditorGrid: "w-full justify-center",
  inlineEditorInvalid: "gap-[7px]",
  inlineFields:
    "inline-flex min-w-0 max-w-full items-center overflow-hidden rounded-[5px] border border-charcoal-active bg-charcoal-bg shadow-none",
  inlineFieldsGrid: "w-full",
  inlineFieldsInvalid: "border-charcoal-active shadow-none",
  inlineFieldsInvalidTable: "max-w-[174px]",
  inlineInput:
    "h-7 w-[min(210px,100%)] min-w-16 border-0 bg-transparent px-[7px] text-cream outline-0",
  lockedExtension: "flex-none py-0 pl-0 pr-[7px] text-cream-muted",
  passiveDraft:
    "inline-flex min-w-0 max-w-full items-center gap-0 overflow-hidden rounded-[5px] border border-charcoal-border bg-charcoal-card px-1.5 py-[3px] text-cream",
  passiveDraftInvalid: "border-charcoal-active text-cream-muted",
  passiveDraftText: "min-w-0 overflow-hidden text-ellipsis whitespace-nowrap",
  passiveDraftExtension: "flex-none text-[inherit] text-cream-muted",
  passiveDraftCaret:
    "ml-0.5 h-4 w-px flex-none animate-[passive-rename-caret_1.1s_step-end_infinite] bg-cream-muted opacity-75",
  footer: cn(
    "flex min-h-9 min-w-0 items-center justify-between gap-3 overflow-hidden",
    "border-t border-transparent bg-transparent px-3 py-1.5 text-xs",
    "text-cream-muted @max-[640px]:min-h-8 @max-[640px]:px-2.5 @max-[640px]:py-0",
    "@max-[640px]:text-[11px]",
  ),
  footerGroup: "flex min-w-0 items-center gap-2 overflow-hidden",
  footerItem: "min-w-0 overflow-hidden text-ellipsis whitespace-nowrap",
  empty: "p-6 text-cream-muted",
  emptyFolder:
    "pointer-events-none absolute inset-x-0 top-10 bottom-9 grid place-items-center px-6 text-center text-[13px] text-cream-muted",
  emptyError: "text-cream-muted",
} as const;
