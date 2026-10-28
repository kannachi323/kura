# Kuragine

Kuragine is Kura's file engine: a Rust crate (`src-tauri/crates/kuragine`) with no dependency on Tauri. The app calls it from thin Tauri commands, and the frontend only awaits those commands and listens for events.

It talks to Google Drive, OneDrive and Dropbox directly. There is no rclone and no Kura server. LAN discovery and device-to-device transfer will live here too; they are not built yet.

## Bring your own OAuth app

Kura ships no client IDs or secrets. Each person registers their own app with the provider and pastes its credentials into Kura:

| Provider | Credentials | Redirect URI to register |
|---|---|---|
| Google Drive | Desktop-app client ID and secret | none (Google allows any loopback port) |
| OneDrive | Application (client) ID; public client flows on | `http://localhost` (Mobile and desktop platform) |
| Dropbox | App key | `http://localhost:53682/` |

Sign-in uses the authorization code flow with PKCE and a loopback redirect. Kura opens the browser, listens once on `127.0.0.1`, and swaps the code for tokens. The credentials and the refresh token are stored together as one item in the OS keychain (`dev.kura.kuragine`, account = remote name). Access tokens are kept only in memory. Google must be set to **In production** in the consent screen, otherwise its refresh tokens expire after 7 days.

## Virtual paths

Every remote has a folder at `~/.kura/providers/<remote>/`. A cloud file's Kura path is the path it would have there, for example `~/.kura/providers/work-drive/Reports/q3.xlsx`. The explorer, tabs, the transfer queue and undo all work with these ordinary paths. A file only exists on disk once it has been downloaded.

Kuragine keeps an index in `~/.kura/db/kuragine.db` (SQLite):

- `items`: one row per known remote item, keyed by `(remote, path)`, with its provider ID, revision, content hash and size. Path lookups are local, so opening `/a/b/c` costs no API calls once the folders have been listed.
- `copies`: one row per downloaded file. It records the **base**: the content hash and revision at download, plus the file's size and modified time as Kuragine left them.
- `conflicts`: uploads that stopped because the cloud copy changed.

Names are made safe for the local disk:
- `/` and other characters a filesystem rejects are swapped for look-alikes.
- Names that differ only in case, or are exact duplicates (Drive allows both), get ` (2)`, ` (3)` suffixes, ordered by item ID so the suffixes stay stable.
- Google Docs, Sheets, Slides and Drawings appear with an Office or PNG extension. They are read-only exports.

Folder listings are served from the index immediately and refreshed in the background. A `cloud://listing-changed` event tells the window to reload.

## Opening and freshness

Opening a cloud file:

1. **Not downloaded:** download it to a temporary file. Kuragine verifies the provider's hash while streaming, sets the file's modified time to the cloud's, then moves it into place.
2. **Downloaded:** return it at once. In the background, ask the provider for the item's current hash.

| Local file vs base | Cloud vs base | Result |
|---|---|---|
| same | same | nothing |
| same | changed | the local copy is replaced (atomic rename) |
| changed | either | left alone; the upload decides |

Each provider uses its own hash, and Kuragine computes the same kind locally:
- **Google Drive:** MD5
- **Dropbox:** `content_hash` (SHA-256 of 4 MiB block SHA-256s)
- **OneDrive:** QuickXorHash

## Uploading local changes

A filesystem watcher (`notify`) covers `~/.kura/providers`. Watching for closed file handles isn't possible across apps, and editors save by writing a temporary file and renaming it, so Kuragine waits for a path to stay quiet for 1.5 seconds. It then:

1. Skips editor junk (`~$x.docx`, `.swp`, `.DS_Store`, `*.tmp`, …), folders, missing files and read-only exports.
2. Skips the file if its size and modified time still match the copy record. If they differ, it hashes the file and skips it when the hash equals the base.
3. Otherwise asks the app to queue an **Upload** transfer.

The upload checks the cloud before writing:

1. Fetch the item's current metadata (one request).
2. If its content differs from the base, it's a **conflict**. The cloud version is downloaded to `~/.kura/.cache/cloud-conflicts/`, and the transfer waits for a decision.
3. Otherwise upload, using the provider's precondition where one exists. Dropbox uses `mode: update` with the base rev, and OneDrive uses `If-Match` with the eTag that was just fetched. Drive has none, so a tiny window remains.

A conflict appears in Transfers like any other paste conflict. The dialog can open the two versions side by side in Kura's compare view. The choices are:

- **Keep mine:** overwrite the cloud copy.
- **Keep theirs:** the local edit is moved aside to the conflicts folder, never deleted, and the cloud version takes its place.
- **Keep both:** the local edit is uploaded as `name (conflict 2026-10-08 1412).ext`.

Rules that keep this from turning into sync:

- Changes from other apps only ever upload content. Deleting or renaming in Finder never touches the cloud.
- Renames, moves, deletes and new folders made in Kura are explicit cloud operations through the transfer queue, and renames and moves can be undone.
- Kuragine never overwrites a locally modified file. Anything it would replace is moved to the conflicts folder first.

## Errors and limits

- Requests retry with exponential backoff and jitter on 429, 5xx and Drive's `rateLimitExceeded`, honouring `Retry-After`.
- A 401 refreshes the access token once. If the refresh fails, the remote is marked as needing sign-in.
- Uploads are chunked within each provider's rules:

| Provider | Single request | Larger files |
|---|---|---|
| Google Drive | — | resumable session, 8 MiB chunks (multiple of 256 KiB) |
| OneDrive | under 4 MiB | upload session, 10 MiB chunks (multiple of 320 KiB) |
| Dropbox | 8 MiB or less | upload session, 8 MiB chunks |

## Not built yet

- LAN device discovery and transfer: pairing, keys, mDNS.
- Google shared drives and "Shared with me".
- Undo of an automatic upload (restoring the previous cloud revision).
