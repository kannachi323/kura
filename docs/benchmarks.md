# Benchmarks

Numbers here come from the tests in this repo, so anyone can rerun them.

## Directory listing

`list_directory` reads a folder and returns its entries with names, kinds, sizes and
modified times. The benchmark creates folders of 1,000, 10,000 and 100,000 entries
(one in twenty is a subfolder), does one untimed pass to warm the OS file cache, then
times 15 runs and reports the median and the 95th percentile.

Run it:

```bash
cargo test --release --manifest-path src-tauri/Cargo.toml listing_benchmark -- --ignored --nocapture
```

The test lives in `src-tauri/src/infra/explorer/tests_perf.rs`. It is ignored in normal
test runs because it creates up to 100,000 files.

### Results

Machine: AMD Ryzen 5 5600X (6 cores, 12 threads), 32 GB RAM, T-FORCE SSD, Windows 11 Pro,
Rust 1.91, release build.

| Date | Change | Entries | Median ms | p95 ms | Entries/sec |
| --- | --- | ---: | ---: | ---: | ---: |
| 2026-10-24 | Baseline | 1,000 | 40.8 | 42.8 | 24,519 |
| 2026-10-24 | Baseline | 10,000 | 414.4 | 431.1 | 24,132 |
| 2026-10-24 | Baseline | 100,000 | 4,149.5 | 4,206.5 | 24,099 |
| 2026-10-24 | Read each entry's metadata from the directory read instead of a second stat call | 1,000 | 2.6 | 3.8 | 386,414 |
| 2026-10-24 | Read each entry's metadata from the directory read instead of a second stat call | 10,000 | 21.9 | 29.0 | 456,390 |
| 2026-10-24 | Read each entry's metadata from the directory read instead of a second stat call | 100,000 | 212.5 | 254.8 | 470,540 |

Listing time grows in a straight line with the number of entries, at about 41 µs per
entry at baseline, so the cost was in per-entry work, not in reading the directory.

Timing the phases separately at 100,000 entries showed why: the directory read itself takes
about 56 ms, but a second `symlink_metadata` call per entry took about 3.7 s. `DirEntry::metadata()`
returns the same data (it does not follow symlinks) without that call, which made a 100,000-entry
listing 19.5x faster (4,150 ms to 212 ms).

Add a row for every change that moves these numbers, with the date and what changed.
