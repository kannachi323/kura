# Kura

[![CI](https://github.com/kannachi323/kura/actions/workflows/ci.yml/badge.svg)](https://github.com/kannachi323/kura/actions/workflows/ci.yml)

Kura is a free, open-source file manager for macOS, Windows and Linux. It works entirely on your device: no account, no sign-in and no cloud service.

- Tabs, split panes, list and grid views, and a preview panel for images, video, audio, PDFs, documents and archives.
- Copy, move, rename and delete through a queue you can pause, resume, cancel, retry and undo. Transfers keeps the history.
- Private local search with an on-device index.
- Archives (list, create ZIP, extract), duplicate finder, file and folder compare, checksums and symlinks.
- Folder sync pairs, Open With choices per file type, and external and network volumes.
- Opens from other apps with `kura://open?path=…&select=…` and `kura://search?q=…`.

## Performance

Listing a folder of 100,000 entries takes about 0.2 seconds, 19x faster than the first
version. Each entry's metadata now comes from the directory read itself instead of a second
stat call per file.

| Entries in folder | Before | After |
| ---: | ---: | ---: |
| 1,000 | 40.8 ms | 2.6 ms |
| 10,000 | 414 ms | 21.9 ms |
| 100,000 | 4,150 ms | 212 ms |

Median of 15 runs, release build, Ryzen 5 5600X, SSD, Windows 11. The benchmark is part of the
test suite, so you can rerun it on your machine. See [docs/benchmarks.md](docs/benchmarks.md).

## Develop

Kura uses Tauri 2, React and Rust (the toolchain is pinned in `rust-toolchain.toml`).

```bash
npm install
npm run tauri dev
```

Checks run locally through git hooks. Enable them once:

```bash
git config core.hooksPath .githooks
```

Then `.githooks/checks.sh` runs formatting, type checks, frontend tests and `cargo test`.

Settings, the search index and transfer history live in `~/.kura`.

## Release

Releases build and sign on a Mac with `scripts/release.sh X.Y.Z`. The script lists what it needs: the updater signing key (`npx tauri signer generate`, with its public half in `src-tauri/tauri.conf.json`), an Apple Developer ID and notarization key, and `gh`.

## License

MIT
