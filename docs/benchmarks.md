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

Listing time grows in a straight line with the number of entries, at about 41 µs per
entry, so the cost is in per-entry work, not in reading the directory.

Add a row for every change that moves these numbers, with the date and what changed.
