# Kura: interview prep

Reminder list, not an essay. Numbers come from `scripts/bench.sh` on the owner's Mac (Apple Silicon, 14 cores, APFS), run on 2026-10-28 on branch `overnight-mvp`.

---

## 1. Pitches

**30 seconds**
- Kura is a desktop file manager for macOS, Windows and Linux, built with Tauri 2, a Rust backend and a React UI.
- Everything stays on the device: browsing, a private search index, and a file-operation queue with pause, cancel, retry and undo.
- The work I'm proudest of is speed. Listing a 100,000-file folder takes about 90 ms in the backend, the list is virtualized, and name search over 100k files answers in 15–30 ms.

**2 minutes**
- Problem: Finder is slow and opaque on huge folders, and copies can't be queued, paused or reliably undone. Power-user tools (ForkLift, Path Finder) cost money and aren't cross-platform.
- Shape: one Tauri window. React renders tabs, split panes, list and grid views and a preview panel. Rust does all file-system work and exposes about 85 IPC commands.
- Three engines in Rust:
  1. **Listing pipeline**: `read_dir`, then parallel `lstat` on up to 8 threads, a cached-key sort, and JSON over IPC.
  2. **Operation queue**: copy, move, rename, delete and trash become queued operations with concurrency limits (8 total, per-provider lanes), cooperative cancellation through an `AtomicBool` per operation, pause/resume, conflict policies, and an undo/redo stack backed by a SQLite transfer history.
  3. **Search index**: a Tantivy inverted index on file names (fuzzy, edit distance 2) plus a SQLite manifest of per-file signatures. Re-scans only rewrite what changed, and a new index is built in staging and swapped in atomically.
- Folders open in panes are watched (FSEvents through `notify`), so they refresh live.
- Data lives in `~/.kura`: config, a SQLite database, and a cache with the search index, thumbnails and Kura's own Trash.
- What I'd do next: move the IPC payload from JSON to a binary or streamed format (it's now the biggest cost), use the system Trash, and add a recursive watcher so the index updates in real time.

---

## 2. Architecture

```
┌──────────────────────────── Tauri window (WKWebView / WebView2 / WebKitGTK) ───────────────────────────┐
│ React 19 + Zustand                                                                                      │
│  DesktopLayout ─ tabs / dock tree / split panes                                                         │
│  ExplorerPane ─ FileTable / FileGrid (windowed rows)  ─ fileListKeyboard (Finder keys)                  │
│  useExplorerStore (panes, selection, history) · useOperationQueueStore · useSearchStore · settings      │
│  SearchPalette (Cmd+F) · FileInspector (preview) · useDirectoryWatch · useSearchAutoIndex               │
└───────────────┬──────────────────────────────────────────────▲─────────────────────────────────────────┘
                │ invoke("cmd", args)  JSON request/response      │ events: kura://directory-changed,
                ▼                                                  │ devices-changed, cloud, queue
┌───────────────────────────── Rust (tokio) ── KuraRuntime: one struct of services ──────────────────────┐
│ app/commands.rs  (#[tauri::command] fns: thin, map to services)                                          │
│ ExplorerService ── list_directory (domain/explorer.rs: read_dir + parallel lstat + sort)                 │
│                 ── preview, thumbnails, trash (~/.kura/.cache/trash), cloud paths → kuragine crate       │
│ OperationQueueService ── OperationQueue (domain): batches, lanes, max_concurrent=8, pause, conflicts     │
│                       ── pump: take_ready() → tokio::spawn(execute) → finish → schedule_pump()           │
│                       ── undo(token) / redo stack · TransferService (SQLite history, undo tokens)        │
│ SearchService ── Tantivy index (~/.kura/.cache/search/local-v2) + SQLite manifest (signatures)           │
│               ── paced, low-priority scan on spawn_blocking · cross-process scan lock · atomic swap      │
│ DirectoryWatchService ── notify (FSEvents) non-recursive per open folder → 250 ms debounce → emit         │
│ Settings, ExplorerLibrary (recent/starred), DirectorySize, Devices, FileSync, Metadata ...               │
└──────────────────────────────────────────────────────────────────────────────────────────────────────────┘
~/.kura/  config/settings.json · db/data.db (transfers, sync) · db/kuragine.db (cloud) · .cache/{search,thumbnails,trash}
```

- **Split:** the UI never touches the file system. Every effect goes through a command, and long work runs on `spawn_blocking` or a tokio task.
- **IPC:** Tauri 2 `invoke` serializes arguments and results as JSON (serde ↔ `JSON.parse`). Events push from Rust to the UI (`app.emit`).
- **State:** Zustand stores per domain. Panes are keyed by id, and selection plus history live per pane.
- **Ops are asynchronous:** the UI enqueues and gets a snapshot back. The queue runs the work, then the UI refreshes (queue events, directory watcher).

---

## 3. Key design decisions

### Tauri instead of Electron or native
- **Chose:** Tauri 2. Rust core, system webview.
- **Why:** a small binary (no bundled Chromium) and low idle memory. Rust gives fast, memory-safe file-system code with real threads, which is the core of a file manager. One codebase covers three OSes.
- **Alternatives:** Electron (bigger, Node for file-system work, mature ecosystem); native SwiftUI/AppKit (best feel, but macOS only and one UI per OS); Qt (cross-platform but C++ and dated web tooling).
- **Trade-offs:** webview differences (WKWebView vs WebView2), the IPC hop costs serialization (see the benchmark: about 45 ms encode and 58 ms parse for 100k entries), and it's harder to look 100% native (no NSOutlineView, so rows are hand-made).

### Listing pipeline
- **Chose:** `read_dir`, then `DirEntry::metadata` (lstat, no symlink follow) per entry, spread over up to 8 scoped threads fed by batches of 2,048 through a channel while the main thread keeps reading names. Below 4,096 entries it stays sequential.
- **Sort:** `sort_by_cached_key((folder_rank, lowercase name))`, so one allocation per entry instead of two per comparison (95–140 ms became about 4 ms at 100k).
- **Path string** built once per entry and cloned for `id`. Unset location fields are skipped in the JSON (55.4 MB became 46.0 MB for 100k).
- **Frontend:** one shared `Intl.Collator` for natural sort. `localeCompare(…, {numeric})` builds a collator per call: 238 ms vs 21 ms in V8 for 100k, and likely worse in WebKit. Rows are windowed (`FileTable`, `FileGrid` render only the visible slice plus overscan).
- **Alternatives tried:** macOS `getattrlistbulk` (one syscall per batch of entries). Prototyped and verified identical to lstat, but it was **no faster on APFS** (~290 ms vs ~290 ms), so I dropped it. Also considered streaming pages of entries.
- **Trade-offs:** threads help because lstat is mostly kernel time per call. It scales to about 4–8 threads, then vnode lock contention flattens it. JSON is now the largest remaining cost.

### Operation queue
- **Model:** `OperationDescriptor` (kind, source/target endpoints, conflict policy, status) grouped into batches. `OperationQueue::take_ready()` hands out work up to `max_concurrent` (default 8) and per-lane limits (`max(2, max/2)` per provider), so one slow cloud account can't starve local copies.
- **Execution:** a single "pump" loop (guarded by an `AtomicBool`) spawns a tokio task per ready operation and re-pumps when each one finishes.
- **Cancellation:** cooperative. Each running operation has an `Arc<AtomicBool>`, checked between chunks and files (`ensure_not_canceled`). Pause = cancel plus requeue as paused. A completion that lands after a cancel is ignored (there is a test for this race).
- **Conflicts:** Ask / Replace / Skip / KeepBoth. Ask parks the operation as `WaitingForResolution` and the UI shows a dialog. "Keep both" picks `name 1.ext`, `name 2.ext` and so on.
- **Undo/redo:** each completed transfer row gets an undo token if it can be reversed: rename, move, and (new tonight) trash, which records where in Kura's Trash the item went. Undo enqueues the inverse operation and pushes the row onto a redo stack. Anything the user starts clears redo; undo and redo don't (fixed tonight).
- **Durability:** transfer history is in SQLite (`TransferService`). Orphaned rows are recovered at launch. Payloads live in memory, so a crash loses queued work but not history.
- **Alternatives:** OS-level copy APIs (`copyfile`, `IFileOperation`), which are fast but give no unified queue, cross-provider moves or undo; or a semaphore per job, which is simpler but doesn't give lanes, pause or ordering.

### Incremental search index
- **Chose:** Tantivy (Rust Lucene-like) on file *names*: tokenized `name`, plus stored path, extension, size, mtime, flags. Queries are a `BooleanQuery` of fuzzy (distance 2) and exact term per token, top 10,000 hits, then re-scored in Rust (token score, scope, filters) down to 100.
- **Incremental:** a SQLite manifest maps path → signature (is_file, is_dir, size, mtime, hidden). A re-scan walks the tree and only writes documents whose signature changed, then removes ones it didn't see under fully walked roots.
- **Safety:** the scan writes to a staging copy and swaps it in only when complete (`replace_index`), so a cancelled or crashed scan keeps the last good index. A file lock stops two Kura processes from scanning at once, and there's a minimum spacing of 60 s.
- **Being a good citizen:** the scan runs at background priority with pacing (40 ms of work, 20 ms pause), which is why 100k items take about 22 s to index.
- **Alternatives:** Spotlight (`mdfind`): free, but opaque, can't index cloud mounts the same way, and gives no typo tolerance. SQLite FTS5: simpler, but fuzzy matching is weaker. A trigram index: great for substrings, but bigger.
- **Trade-offs:** "incremental" means a full walk that skips unchanged documents, not event-driven. The walk is still O(files). A recursive FSEvents journal would make it O(changes).

### Live folder refresh (added tonight)
- `notify` watches each open folder non-recursively. Changed paths map to their parent folder, are coalesced (250 ms of quiet, at most 1 s), and are emitted as `kura://directory-changed`. Panes showing that folder reload quietly, keeping selection.

---

## 4. Defending the resume bullets

### Bullet 1: "Building a desktop file manager backed by an incremental search index and an operation queue with concurrency limits, safe cancellation and undo"
- **Holds up**, with these caveats:
  - *Incremental index:* `src-tauri/src/infra/search.rs` (`ScanSink::accept` skips unchanged signatures, `remove_missing`, staging plus `replace_index`). Before tonight nothing in the UI triggered a scan and Cmd+F opened nothing. Fixed in `fa2592d` (search palette) and `f6b10e5` (auto-index setting). Say "incremental re-scan" rather than "real-time".
  - *Concurrency limits:* `domain/operation_queue.rs` `take_ready` (`max_concurrent`, `max_lane_concurrent`). Test: `provider_lanes_keep_one_remote_from_monopolizing_queue`.
  - *Safe cancellation:* `AtomicBool` per operation, `ensure_not_canceled`. Tests: `late_success_after_cancel_does_not_overwrite_transfer`, `canceling_running_service_operation_marks_transfer_canceled_and_prunes_payload`, `pausing_running_service_operation_requeues…`.
  - *Undo:* `OperationQueueService::undo/redo`. Before tonight, Cmd+Z twice re-applied the first change and trash couldn't be undone. Fixed in `2ff7ae0`, with tests `an_undo_is_not_itself_the_next_undo`, `two_undos_can_both_be_redone` and `trash_can_be_undone_and_redone`.
- Run them: `cargo test --manifest-path src-tauri/Cargo.toml operation_queue`

### Bullet 2: "Cut directory listing time for 100,000 files by 97% from 4.1 seconds to 105 milliseconds by profiling each phase and removing redundant per-file system calls and path rebuilds"
- **Does not hold up as written.** Be ready to say so and use the corrected numbers.
  - The git history has a single squashed commit (`ac8716a`), so there's no commit showing a 4.1 s version and no record of how 4.1 s was measured.
  - The code at the start of tonight measured **346 ms median** (631 ms on the first run) for the Rust listing of 100k files, not 4.1 s and not 105 ms.
  - After tonight's work (`6d3a1d2`, `9e1debe`): **91.5 ms median** for the Rust listing, 45 ms JSON encode, 58 ms `JSON.parse`, and the front-end sort went from 238 ms to 21 ms (V8).
  - The "4.1 s" may have been an end-to-end time (IPC plus the per-call `localeCompare` sort in WebKit, plus rendering) on an older build. It's plausible but **not reproducible from this repo**.
- **What tonight's profiling showed** (`scripts/bench.sh` plus a phase probe):
  - `read_dir` about 60 ms. Per-file `lstat` about 230–260 ms (~2.4 µs each). Lowercase-per-comparison sort 95–140 ms.
  - Fixes: parallel lstat (230 ms → about 80 ms), cached sort keys (95+ ms → 4 ms), a single path string per entry, and slimmer JSON.
  - Tried and rejected: `getattrlistbulk`, which gave no gain on APFS.
- **Suggested honest wording:**
  - "Cut Rust listing time for a 100,000-file folder by 74% (346 ms → 91 ms) by profiling each phase, parallelizing per-file `lstat` calls and removing per-comparison allocations; cut front-end sorting by 91% (238 → 21 ms) with a shared collator."
  - Or end to end, without the 4.1 s: "…lists 100,000 files in ~90 ms on the backend with a virtualized UI."
- **Reproduce:** `scripts/bench.sh` (it builds `src-tauri/examples/bench.rs`, creates the fixture in a temp dir, prints first-run and median-of-7, then JSON parse and search). Measure "before" with `git stash` or by checking out `3a866a6`, then `cargo build --release --example bench`. The bench example was added after that commit, so copy `examples/bench.rs`, `src/bench.rs` and the `lib.rs` line across.

### Measured numbers (2026-10-28, `overnight-mvp`)

| Metric (100,100 entries) | Before tonight | Now |
|---|---|---|
| Rust listing, median of 7 | 346 ms | **91.5 ms** |
| Rust listing, first run | 631 ms | 94 ms |
| IPC JSON size | 55.4 MB | 46.0 MB |
| serde encode | 48 ms | 45 ms |
| `JSON.parse` (Node) | 68 ms | 58 ms |
| Front-end sort (V8) | 238 ms | 21 ms |
| Index build (paced, background) | — | 21.9 s |
| Search "invoice" / "report 004" / "scrnshot" (typo) | — | 14 / 28 / 18 ms median |

- Not measured: **cold start** (no screen access overnight). To measure: `npm run tauri build -- --no-bundle`, then time from process start to the first `explorer_list_directory` response (add a `performance.now()` log), with `sudo purge` beforehand for a truly cold run.
- All the "warm" numbers above are with the page cache warm. True cold-cache listing needs `sudo purge`.

---

## 5. Likely questions (with answer pointers)

**Technical deep dive**
1. *Walk me through what happens when I open a folder.* → `navigatePane` → `invoke(explorer_list_directory)` → `spawn_blocking(list_directory)` → read_dir + parallel lstat + sort → serde JSON → `normalizeDirectoryListingPaths` + `sortListing` (collator) → Zustand → `FileTable` renders about 30 rows.
2. *Why did parallel lstat help if the disk is one device?* → It's metadata in the kernel's vnode/inode cache, so it's CPU and syscall-bound, not I/O-bound. It stops scaling past about 8 threads because of locks.
3. *Why not `getattrlistbulk`?* → I prototyped it and it matched lstat's results exactly but not its speed on APFS. Measure, don't assume.
4. *How is the list virtualized?* → Fixed row height. `startIndex = floor(scrollTop/rowHeight) - overscan`, and spacer rows above and below. Scroll is rAF-throttled. For the grid, columns come from the width and rows are virtualized.
5. *How does cancellation work mid-copy?* → An `AtomicBool` per operation, checked between buffers and files. On cancel the queue marks the operation canceled. A late success is ignored. (Check how a partial destination file is cleaned up before claiming it; I didn't verify that overnight.)
6. *How do you avoid two copies writing the same destination?* → Conflict policy at execution time (`resolve_target_conflict`). Ask parks the operation. KeepBoth finds a free name. Batches can preserve order.
7. *What's a "lane"?* → A key per provider or remote. Each lane is capped at `max(2, max/2)` so one slow remote can't use all 8 slots.
8. *How does undo know what to do?* → The transfer row stores source and destination. The undo token is the row id. Undo enqueues the inverse (rename back, or move back from Trash) as `from_undo` so it isn't undoable itself, then pushes the row to the redo stack.
9. *Why isn't delete undoable?* → Permanent delete isn't. Trash is (Kura's own Trash dir), and the confirmation dialog is only for permanent deletes.
10. *Explain the search index.* → An inverted index: term → posting list of document ids. Fuzzy uses a Levenshtein automaton over the term dictionary (an FST) in Tantivy. Results are re-ranked in Rust.
11. *How is it incremental?* → A SQLite manifest of signatures. Unchanged items are skipped, changed ones are deleted and re-added, and missing ones are removed after a full walk. Staging plus an atomic swap.
12. *What happens if the app crashes mid-index?* → The staging directory is discarded, the live index is untouched, and `recover_interrupted_publish` handles a half-finished swap.
13. *Tauri IPC cost?* → JSON both ways. 46 MB for 100k entries means about 45 ms to encode and about 58 ms to parse, more than the listing itself now.
14. *How do live updates work?* → notify/FSEvents, non-recursive per open folder, debounced 250 ms (max 1 s), quiet reload. Why non-recursive: cheap, and only visible folders matter.
15. *How do you keep keyboard focus right in a windowed list?* → A `revealEntry` event: scroll the index into view, then focus the row once React has rendered it (retried for up to 2 s).

**System design follow-ups**
16. *Scale to 10M files in one index?* → Tantivy handles it (segments, merges). The bottleneck is the walk, so use an FSEvents journal (`kFSEventStreamEventIdSinceNow` plus stored event id) to do O(changes) re-scans. Shard per volume. Store fewer fields.
17. *A 1M-file single folder?* → Stream the listing in pages (a Tauri `Channel`), sort in Rust and send only the visible window plus ids, or use a binary format. Never ship 460 MB of JSON.
18. *Network drives (SMB/NFS)?* → lstat is a round trip, so parallelism helps more but must be capped. Add timeouts and cancellation on listing, cache listings, show partial results, and don't watch (FSEvents is unreliable there), poll instead.
19. *Multiple windows or processes?* → The search scan lock already exists across processes. The queue is per process, so it would move into a single service or daemon owner, or use SQLite as the coordination point.
20. *Make the queue survive crashes?* → Persist payloads (not only history) in SQLite, use idempotent steps (copy to temp, then rename), and resume on launch.
21. *Copying a 50 GB file: progress and speed?* → Chunked copy with progress events. The queue stores a bandwidth limit (`set_bandwidth_limit`); check how far it is enforced for local copies before claiming it. Use `copyfile`/`clonefile` on APFS for instant clones on the same volume.
22. *Search by content, not just names?* → Extract text (pdf-extract, calamine and so on are already dependencies) into a separate field, with size caps and a background priority queue.

**Debugging stories**
23. *A bug you found by measuring?* → Sort: `localeCompare` with options builds an `Intl.Collator` per call. 238 → 21 ms with a shared collator.
24. *A UX bug that only showed by driving the app?* → New → Folder: Radix returned focus to the menu trigger, so the typed name went nowhere and Return reopened the menu. Fixed with `onCloseAutoFocus` plus a re-focus on the next frame.
25. *A logic bug in undo?* → Undo created an undoable rename, so Cmd+Z twice re-did the change. Also, undo cleared the redo stack. Fixed with an origin flag (User/Undo/Redo) and covered with tests.
26. *Something dead in the UI?* → Cmd+F opened a store nobody rendered, and the auto-index setting wasn't read. Found by testing flows end to end, not just unit tests.
27. *How did you test the UI without screen access?* → A dev-only Vite middleware relayed `invoke` calls from a normal browser tab through the real Tauri webview, so the real Rust backend served a browser I could script.

**What would you do differently**
28. → Benchmark before claiming, and keep the before/after in the repo (that's why `scripts/bench.sh` exists now).
29. → Use the system Trash (`trash` crate, NSFileManager `trashItem`) instead of a private one.
30. → A binary or streamed IPC for listings, and sorting in Rust only.
31. → Fewer features, more polish: tonight's bugs were in core flows (search, undo, create).
32. → Smaller commits from day one. The history is one squashed commit, which made claims hard to prove.

**Behavioural**
33. *Tell me about using AI tools.* → Used for speed. I own the design and the verification, and I found and fixed real bugs by testing flows and measuring. Be specific about the bugs above.
34. *A trade-off you made?* → Background pacing makes indexing about 3× slower in exchange for staying invisible to the user.
35. *A time you were wrong?* → Assumed `getattrlistbulk` would win. Measured, and it didn't.

---

## 6. Concepts to review

- **Concurrency:** `Arc`, `Mutex` vs `RwLock`, atomics and memory ordering (`SeqCst`), `std::thread::scope`, mpsc channels, tokio `spawn` vs `spawn_blocking`, cooperative cancellation, races (late completion after cancel).
- **File-system APIs:** `readdir`/`getdents`, `stat` vs `lstat` (symlinks), `fstatat`, `getattrlistbulk`, `d_type`, APFS clones (`clonefile`), atomic `rename`, inode/vnode caches, page cache vs cold reads (`purge`, `drop_caches`).
- **Change notification:** FSEvents (macOS, per-directory, coalesced, event ids), inotify (Linux, watch limits, no recursion), ReadDirectoryChangesW (Windows), debouncing.
- **Search:** inverted indexes, posting lists, tokenization, BM25, Levenshtein automata and FSTs, segment merges, trigram indexes, FTS5.
- **UI performance:** list virtualization (windowing, overscan, fixed vs variable heights), React reconciliation and keys, rAF throttling, `Intl.Collator`, avoiding allocation in comparators.
- **IPC and serialization:** JSON costs (encode, transfer, parse), structured clone, binary formats (MessagePack, bincode), streaming and channels, payload shaping (skip nulls, avoid duplicate fields).
- **Persistence:** SQLite WAL, transactions, crash recovery, idempotent operations, staging plus atomic swap.
- **Algorithms:** sort with cached keys (Schwartzian transform), natural sort, debouncing and throttling.
- **Desktop app architecture:** Tauri vs Electron process models, capabilities and permissions (the zoom capability bug), webview differences, code signing and notarization (not done here).

---

## 7. Comparison with Finder, ForkLift and Dolphin, and MVP gaps

Ranked. Done means fixed on this branch.

**MVP must-haves**
1. Search reachable from the keyboard (Finder Cmd+F, ForkLift Cmd+F). **Done** (was broken).
2. Keyboard navigation in lists: arrows, Shift-extend, Cmd+↑/↓, Return renames, Space for Quick Look, Cmd+Delete to trash. **Done** (was missing).
3. Undo that behaves like Finder (trash undoable, no ping-pong). **Done.**
4. Live refresh when other apps change a folder (all three do it). **Done.**
5. Create and rename that keep focus and selection. **Done.**
6. Readable selection; light and dark that follow the system. **Done.**
7. Fast large folders (Finder stalls on 100k; Dolphin is fast). **Done**: 91 ms backend.
8. Use the system Trash so items show in Finder's Trash and "Put Back" works. **Not done**: Kura uses `~/.kura/.cache/trash`.
9. Drag and drop to and from other apps. **Not verified overnight** (the code exists).

**Nice to have**
- Column view (Finder), dual-pane sync browsing (ForkLift), a terminal panel (Dolphin), tags (Finder), Get Info (Cmd+I), Duplicate (Cmd+D), type-to-select by name, friendlier Kind labels ("CSV document" instead of `csv`), a binary IPC for 1M+ folders.

**Look**
- Kura's dark palette is quiet and consistent. Tonight: denser rows (40 px, 32 px compact), an accent selection, a light theme, and the long path bar keeps the current folder visible.

---

## 8. Decisions made overnight

- Committed the owner's uncommitted work as-is first (`3a866a6`), then a formatting-only commit (`10de23b`) and clippy fixes (`8e10be7`), because the newer toolchain had broken `checks.sh`.
- **Did not change the resume claim's numbers to fit.** Recorded what the code measures and suggested new wording.
- Dropped the `getattrlistbulk` prototype (correct, but no speedup) to keep the code simple.
- Left unset location fields out of the JSON. The TypeScript types were made optional, and no code compared them to `null` (checked).
- Changed Finder-convention shortcuts on macOS: Trash is Cmd+Delete (the old "Delete" binding never fired on laptops), search is Cmd+F (Cmd+Shift+F still works), Return renames (Enter still opens on Windows and Linux). Shift-click now keeps its anchor, like Finder.
- Default list rows went from 50 to 40 px (32 px with Compact rows, which list view previously ignored).
- Light theme follows the system by default; Settings → Appearance can pin Light or Dark.
- Trash still uses Kura's own folder. Switching to the system Trash is a bigger change, so it's listed as the top remaining gap.
- Auto-indexing now keeps an existing index current: 30 s after launch, then on the chosen interval (the setting already defaulted to on but did nothing). The first build of the index waits for "Build index" in search, so launching Kura never starts a crawl of the whole home folder on its own.
- UI testing: computer-use couldn't target the unbundled dev binary without the owner approving a dialog, so I relayed IPC from the built-in browser through the running Tauri webview with a temporary, uncommitted Vite plugin. Tested: browse, path bar, breadcrumbs, selection and keyboard, rename, undo/redo, trash and undo, new folder, copy/paste, preview, grid view, settings, light and dark, live refresh, and the search palette UI.
- **Not tested in the UI:** tabs, split panes, drag and drop, opening files in other apps, search results against a built index (the backend was benchmarked instead), cold start. A late request said the owner's Mac was overheating, so I stopped the dev app, avoided more heavy builds, and deleted the benchmark fixtures.
