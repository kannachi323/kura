//! Directory-listing benchmark. Ignored by default; run with
//! `cargo test --release --manifest-path src-tauri/Cargo.toml listing_benchmark -- --ignored --nocapture`
//! and see `docs/benchmarks.md` for how to read the table it prints.
use super::*;
use std::time::Instant;

const SIZES: [usize; 3] = [1_000, 10_000, 100_000];
const RUNS: usize = 15;

fn populate(root: &Path, count: usize) {
    std::fs::create_dir_all(root).unwrap();
    for index in 0..count {
        if index % 20 == 0 {
            std::fs::create_dir(root.join(format!("folder-{index:06}"))).unwrap();
        } else {
            std::fs::write(root.join(format!("file-{index:06}.txt")), b"x").unwrap();
        }
    }
}

fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    let rank = ((sorted.len() as f64 - 1.0) * fraction).round() as usize;
    sorted[rank]
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "benchmark: run explicitly with --ignored --nocapture"]
async fn listing_benchmark() {
    println!("| entries | median ms | p95 ms | entries/sec (median) |");
    println!("| ---: | ---: | ---: | ---: |");
    for size in SIZES {
        let root = unique_test_dir("listing-benchmark");
        populate(&root, size);
        let service = test_explorer_service_for_home(root.clone());
        let request = || ListDirectoryRequest {
            path: Some(display_path(&root)),
            show_hidden: Some(false),
            force_remote_refresh: None,
        };

        // One untimed pass warms the OS file cache so runs measure Kura, not the disk.
        let warm = service.list_directory(request()).await.unwrap();
        assert_eq!(warm.entries.len(), size);

        let mut millis = Vec::with_capacity(RUNS);
        for _ in 0..RUNS {
            let started = Instant::now();
            let listing = service.list_directory(request()).await.unwrap();
            millis.push(started.elapsed().as_secs_f64() * 1000.0);
            assert_eq!(listing.entries.len(), size);
        }
        millis.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median = percentile(&millis, 0.5);
        let p95 = percentile(&millis, 0.95);
        println!(
            "| {size} | {median:.1} | {p95:.1} | {:.0} |",
            size as f64 / (median / 1000.0)
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
