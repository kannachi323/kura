# UI polish pass

A final craft pass over every view, aiming for Finder- or Files-style
minimalism. Screenshots are from the running app at 1024×768 (dark unless noted).
Each pair is in `before/` and `after/`.

## What was already right

- **Capitalisation:** no all-caps text anywhere. There are no `uppercase` classes, and section headers, column headers and badges are all sentence case.
- **Fonts:** one system font stack (`system-ui, -apple-system, …`) plus one monospace stack. No web fonts.

## Fixed

| View | Before | After |
|------|--------|-------|
| **List: clipped table in narrow panes** | With the sidebar and inspector open, the table kept its 720 px minimum. Columns were cut off ("Si…") and a horizontal scrollbar appeared. The responsive rules looked at the *window* width, not the pane's. | The table is a container query on its pane. Below 640 px it shows Name and Modified; below 340 px, Name only. No horizontal overflow. |
| **List: type and rows** | Rows used 14 px medium-weight names in 40 px rows, with a 40 px header. | Rows use 13 px regular names (12 px header) in 30 px rows; the icon size follows the zoom level. Breadcrumbs went from 14 to 13 px. |
| **Grid** | Cells stretched to fill the pane: a 60 px folder icon sat in a roughly 180 px cell, two columns, large gaps. | Fixed cell widths per size (96, 120 or 148 px), and more columns as the pane widens. Labels are 12 px regular. |
| **Empty folder** | A blank table and "0 items". | "This folder is empty." centred. Also "No items match this filter." and "The Trash is empty." |
| **Inspector (Get Info)** | Each label stacked above a 14 px medium value; long paths ran on. | Label and value side by side, 12 px, labels right-aligned, as in Finder's Get Info. Items appears only for folders. |
| **Quick Look** | The details panel overflowed horizontally (the path was cut off, with a scrollbar). Markdown rendered as unstyled text: the `prose` classes needed a typography plugin that isn't installed. | Compact label/value details that wrap. A small Markdown stylesheet (`.kura-markdown`) on the theme tokens styles headings, lists, code, quotes and tables, in light and dark. |
| **Transfers** | Source and Destination showed absolute paths cut off at the start, so Destination read "Des…". | Folders shown short: `~/.Trash`, `…/scratchpad/sandbox`. The full path is on hover. |
| **Settings** | Each nav item had a chevron right after its label, which didn't expand anything. Default view offered List and Grid only. | No chevrons. Default view also offers Columns. |
| **Narrow window (600 px)** | The sidebar, list and inspector were squeezed together. The toolbar's actions overlapped (Delete was cut off) and dates were clipped. | The inspector steps aside below 760 px wide and returns when the window widens. A narrow toolbar keeps New, with the edit actions in menus and shortcuts. Columns are rebalanced so dates fit. |
| **Light mode** | — | Checked: same layout, theme tokens throughout (`after/02-list-light.jpg`). |
| **Dev reliability** | Vite found lazily imported packages (`@tauri-apps/api/webview`, …) on first use and re-optimised. Open windows then failed to load them, and a raw "Failed to fetch dynamically imported module" toast appeared. | `optimizeDeps.include` pre-bundles them. |

## Checked and left as they are

- **Context menu, search palette (Cmd+F), Tags sidebar, Trash banner, column view and Settings:** every Settings section has content and nothing dead-ends.
- **Tab strip:** the Transfers tab's title only looked missing in one screenshot, caught mid-transition.

## Checks

- **Frontend:** prettier, tsc and vitest: 92 tests.
- **Rust:** cargo fmt, clippy `-D warnings` and cargo test.
- **Tauri production build:** `npm run tauri build -- --no-bundle` (no signing or packaging).

## For the owner

- **Look at it on your own display.** The screenshots come from a 1024×768 browser view of the same UI, so check the real window at your usual size.
- **Bundling is untested.** Signing, notarising and making a `.app` or `.dmg` were deliberately not run.
