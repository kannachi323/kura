//! Listing and search benchmarks. Run through `scripts/bench.sh`.
//!
//!   bench fixture <dir> <count>     create <count> empty files in <dir>
//!   bench list <dir> [runs]         time the listing pipeline and its JSON
//!   bench dump <dir> <out.json>     write the listing JSON (for the JS parse)
//!   bench search <dir> <data-root>  index <dir>, then time queries

use std::{path::PathBuf, time::Instant};

const WORDS: &[&str] = &[
    "report",
    "invoice",
    "photo",
    "notes",
    "draft",
    "budget",
    "meeting",
    "summary",
    "backup",
    "design",
    "screenshot",
    "resume",
    "contract",
    "slides",
    "export",
];
const EXTENSIONS: &[&str] = &["txt", "pdf", "jpg", "png", "md", "docx", "csv", "zip"];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |index: usize| -> &str {
        args.get(index).map(String::as_str).unwrap_or_else(|| {
            eprintln!("usage: see the comment at the top of examples/bench.rs");
            std::process::exit(2)
        })
    };
    match arg(0) {
        "fixture" => fixture(PathBuf::from(arg(1)), arg(2).parse().expect("count")),
        "list" => list(
            PathBuf::from(arg(1)),
            args.get(2).map_or(7, |runs| runs.parse().expect("runs")),
        ),
        "dump" => kura::bench::write_listing_json(&PathBuf::from(arg(1)), &PathBuf::from(arg(2)))
            .expect("dump listing"),
        "search" => search(PathBuf::from(arg(1)), PathBuf::from(arg(2))),
        other => {
            eprintln!("unknown command: {other}");
            std::process::exit(2)
        }
    }
}

fn fixture(dir: PathBuf, count: usize) {
    std::fs::create_dir_all(&dir).expect("create fixture dir");
    let started = Instant::now();
    for index in 0..count {
        let word = WORDS[index % WORDS.len()];
        let extension = EXTENSIONS[(index / WORDS.len()) % EXTENSIONS.len()];
        std::fs::File::create(dir.join(format!("{word}-{index:06}.{extension}")))
            .expect("create fixture file");
    }
    // A few folders so the folders-first sort has work to do.
    for index in 0..(count / 1000).max(1) {
        std::fs::create_dir_all(dir.join(format!("folder-{index:04}"))).expect("create folder");
    }
    println!("fixture: {count} files in {:.2?}", started.elapsed());
}

fn list(dir: PathBuf, runs: usize) {
    let mut lists = Vec::new();
    let mut serializes = Vec::new();
    for run in 0..runs {
        let timing = kura::bench::list_directory(&dir).expect("list directory");
        if run == 0 {
            println!(
                "listing: {} entries, {:.1} MB of IPC JSON",
                timing.entries,
                timing.json_bytes as f64 / 1_000_000.0
            );
            println!(
                "  first run   list {:>7.1} ms  serialize {:>6.1} ms",
                ms(timing.list),
                ms(timing.serialize)
            );
        }
        lists.push(ms(timing.list));
        serializes.push(ms(timing.serialize));
    }
    println!(
        "  median/{runs}  list {:>7.1} ms  serialize {:>6.1} ms  (min list {:.1} ms)",
        median(&mut lists.clone()),
        median(&mut serializes),
        lists.iter().cloned().fold(f64::INFINITY, f64::min)
    );
}

fn search(dir: PathBuf, data_root: PathBuf) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    runtime.block_on(async {
        let bench = kura::bench::SearchBench::new(data_root);
        let started = Instant::now();
        let count = bench.scan(&dir).await.expect("scan");
        println!(
            "index: {count} items in {:.2?} (paced background scan)",
            started.elapsed()
        );
        for query in ["invoice", "report 004", "scrnshot", "budget-012345", "zzzz"] {
            let mut times = Vec::new();
            let mut results = 0;
            for _ in 0..5 {
                let started = Instant::now();
                results = bench.query(query).await.expect("query");
                times.push(ms(started.elapsed()));
            }
            println!(
                "  query {:<16} {:>3} results  median {:>6.1} ms",
                format!("{query:?}"),
                results,
                median(&mut times)
            );
        }
    });
}

fn ms(duration: std::time::Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    values[values.len() / 2]
}
