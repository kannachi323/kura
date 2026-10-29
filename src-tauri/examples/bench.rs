//! Listing and search benchmarks. Run through `scripts/bench.sh`.
//!
//!   bench fixture <dir> <count>     create <count> empty files in <dir>
//!   bench list <dir> [runs]         time the listing pipeline and its JSON
//!   bench dump <dir> <out.json>     write the listing JSON (for the JS parse)
//!   bench search <dir> <data-root>  index <dir>, then time queries
//!   bench rescan <dir> <data-root>  time a full build, then incremental rescans
//!                                   before and after changing 200 files
//!   bench single <dir> <data-root> [trials]
//!                                   median incremental scan after one create,
//!                                   rename or delete vs a full rebuild
//!   bench watch <dir> <data-root> [trials]
//!                                   index <dir>, watch it, then time each
//!                                   single create, rename and delete from the
//!                                   file-system call to the published commit

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
        "fuzzy" => fuzzy(
            PathBuf::from(arg(1)),
            PathBuf::from(arg(2)),
            args.get(3).map_or(20, |runs| runs.parse().expect("runs")),
        ),
        "rescan" => rescan(PathBuf::from(arg(1)), PathBuf::from(arg(2))),
        "single" => single(
            PathBuf::from(arg(1)),
            PathBuf::from(arg(2)),
            args.get(3)
                .map_or(3, |trials| trials.parse().expect("trials")),
        ),
        "watch" => watch(
            PathBuf::from(arg(1)),
            PathBuf::from(arg(2)),
            args.get(3)
                .map_or(20, |trials| trials.parse().expect("trials")),
        ),
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

fn rescan(dir: PathBuf, data_root: PathBuf) {
    // The backend skips an incremental scan within a minute of the last one.
    let spacing = std::time::Duration::from_secs(61);
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    runtime.block_on(async {
        let bench = kura::bench::SearchBench::new(data_root);
        let timed = |label: &'static str, incremental: bool| {
            let bench = &bench;
            let dir = &dir;
            async move {
                let started = Instant::now();
                let counts = bench.scan_with(dir, incremental).await.expect("scan");
                println!(
                    "{label:<28} {:>8.0} ms  indexed {} added {} updated {} removed {} unchanged {}",
                    ms(started.elapsed()),
                    counts.indexed,
                    counts.added,
                    counts.updated,
                    counts.removed,
                    counts.unchanged
                );
            }
        };

        timed("full build", false).await;
        tokio::time::sleep(spacing).await;
        timed("incremental, no changes", true).await;

        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .expect("read fixture")
            .map(|entry| entry.expect("entry").path())
            .filter(|path| path.is_file())
            .collect();
        files.sort();
        for path in &files[..100] {
            std::fs::write(path, b"edited").expect("edit file");
        }
        for path in &files[100..150] {
            std::fs::remove_file(path).expect("delete file");
        }
        for index in 0..50 {
            std::fs::File::create(dir.join(format!("added-{index:03}.txt"))).expect("add file");
        }
        tokio::time::sleep(spacing).await;
        timed("incremental, 200 changed", true).await;
        timed("full rebuild", false).await;
    });
}

fn single(dir: PathBuf, data_root: PathBuf, trials: usize) {
    let spacing = std::time::Duration::from_secs(61);
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    runtime.block_on(async {
        let bench = kura::bench::SearchBench::new(data_root);
        bench.scan(&dir).await.expect("initial build");

        let mut results: Vec<(&str, Vec<f64>)> = Vec::new();
        for (label, op) in [("create", 0), ("rename", 1), ("delete", 2)] {
            let mut times = Vec::new();
            for trial in 0..trials {
                let name = dir.join(format!("single-{label}-{trial}.txt"));
                match op {
                    0 => drop(std::fs::File::create(&name).expect("create")),
                    1 => std::fs::rename(
                        dir.join(format!("single-create-{trial}.txt")),
                        &name,
                    )
                    .expect("rename"),
                    _ => std::fs::remove_file(dir.join(format!("single-rename-{trial}.txt")))
                        .expect("delete"),
                }
                tokio::time::sleep(spacing).await;
                let started = Instant::now();
                let counts = bench.scan_with(&dir, true).await.expect("incremental scan");
                let elapsed = ms(started.elapsed());
                println!(
                    "incremental {label:<6} trial {trial}  {elapsed:>8.0} ms  added {} updated {} removed {} unchanged {}",
                    counts.added, counts.updated, counts.removed, counts.unchanged
                );
                times.push(elapsed);
            }
            results.push((label, times));
        }

        let mut rebuilds = Vec::new();
        for trial in 0..trials {
            let started = Instant::now();
            let counts = bench.scan_with(&dir, false).await.expect("full rebuild");
            let elapsed = ms(started.elapsed());
            println!("full rebuild      trial {trial}  {elapsed:>8.0} ms  indexed {}", counts.indexed);
            rebuilds.push(elapsed);
        }

        for (label, mut times) in results {
            println!("median incremental {label:<6} {:>8.0} ms", median(&mut times));
        }
        println!("median full rebuild       {:>8.0} ms", median(&mut rebuilds));
    });
}

/// Times a single change from the moment its file-system call returns until
/// the watcher has committed and published it, split into event delivery
/// (FSEvents), the debounce wait and the index update itself.
fn watch(dir: PathBuf, data_root: PathBuf, trials: usize) {
    use std::{sync::Mutex, time::Duration};
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    runtime.block_on(async {
        let bench = kura::bench::SearchBench::new(data_root);
        let indexed = bench.scan(&dir).await.expect("initial build");
        println!("watch: {indexed} entries indexed, {trials} trials per operation");
        let (sender, receiver) = std::sync::mpsc::channel();
        let sender = Mutex::new(sender);
        bench
            .watch(&dir, move |update| {
                let _ = sender.lock().expect("sender").send(update);
            })
            .expect("watch");
        // FSEvents takes a moment to start delivering.
        std::thread::sleep(Duration::from_millis(500));
        timer_check("before trials");

        let created = |trial: usize| dir.join(format!("watch-created-{trial:03}.txt"));
        let renamed = |trial: usize| dir.join(format!("watch-renamed-{trial:03}.txt"));
        let mut summary = Vec::new();
        for label in ["create", "rename", "delete"] {
            let mut totals = Vec::new();
            let mut deliveries = Vec::new();
            let mut debounces = Vec::new();
            let mut applies = Vec::new();
            for trial in 0..trials {
                // Start from a quiet watcher: the previous trial's late
                // events must not be counted toward this one.
                while receiver.recv_timeout(Duration::from_millis(500)).is_ok() {}

                let (want_added, want_removed) = match label {
                    "create" => (1, 0),
                    "rename" => (1, 1),
                    _ => (0, 1),
                };
                let started = Instant::now();
                match label {
                    "create" => drop(std::fs::File::create(created(trial)).expect("create")),
                    "rename" => std::fs::rename(created(trial), renamed(trial)).expect("rename"),
                    _ => std::fs::remove_file(renamed(trial)).expect("delete"),
                }
                // A rename may arrive as two batches; time it to the last one.
                let (mut added, mut removed, mut first_event) = (0, 0, None);
                let last = loop {
                    let update = receiver
                        .recv_timeout(Duration::from_secs(10))
                        .expect("the watcher applied no update within 10 s");
                    added += update.added;
                    removed += update.removed;
                    first_event.get_or_insert(update.first_event);
                    if added >= want_added && removed >= want_removed {
                        assert!(update.published, "the change was not published");
                        break update;
                    }
                };
                let first_event = first_event.expect("first event");
                let total = ms(last.committed.duration_since(started));
                let delivery = ms(first_event.saturating_duration_since(started));
                let debounce = ms(last.applying.duration_since(last.first_event));
                let apply = ms(last.committed.duration_since(last.applying));

                // Untimed: the published index answers for the change.
                let (present, absent) = match label {
                    "create" => (Some(created(trial)), None),
                    "rename" => (Some(renamed(trial)), Some(created(trial))),
                    _ => (None, Some(renamed(trial))),
                };
                for (path, expected) in [(present, true), (absent, false)] {
                    let Some(path) = path else { continue };
                    let name = path.file_name().unwrap().to_string_lossy().into_owned();
                    let found = bench
                        .query_paths(&name)
                        .await
                        .expect("query")
                        .contains(&path.display().to_string());
                    assert_eq!(found, expected, "{label} trial {trial}: {name}");
                }
                println!(
                    "{label:<6} trial {trial:>2}  total {total:>7.1} ms  delivery {delivery:>6.1}  debounce {debounce:>6.1}  apply {apply:>6.1}  (added {added} removed {removed})"
                );
                totals.push(total);
                deliveries.push(delivery);
                debounces.push(debounce);
                applies.push(apply);
            }
            summary.push((label, totals, deliveries, debounces, applies));
        }

        timer_check("after trials");
        println!("event-to-commit over {trials} trials (median / p95, nearest rank), ms:");
        for (label, mut totals, mut deliveries, mut debounces, mut applies) in summary {
            println!(
                "  {label:<6} total {:>6.1} / {:>6.1}   delivery {:>5.1} / {:>5.1}   debounce {:>5.1} / {:>5.1}   apply {:>5.1} / {:>5.1}",
                median(&mut totals),
                percentile(&mut totals, 95.0),
                median(&mut deliveries),
                percentile(&mut deliveries, 95.0),
                median(&mut debounces),
                percentile(&mut debounces, 95.0),
                median(&mut applies),
                percentile(&mut applies, 95.0),
            );
        }
    });
}

/// How late a 100 ms timed wait wakes on a fresh thread of this process. The
/// watcher's debounce is such a wait, so its share of the total depends on
/// the machine's timer slack.
fn timer_check(label: &str) {
    let mut waits: Vec<f64> = std::thread::spawn(|| {
        let (_sender, receiver) = std::sync::mpsc::channel::<()>();
        (0..10)
            .map(|_| {
                let started = Instant::now();
                let _ = receiver.recv_timeout(std::time::Duration::from_millis(100));
                ms(started.elapsed())
            })
            .collect()
    })
    .join()
    .expect("timer check");
    println!(
        "timer check {label}: 100 ms wait took median {:.1} ms, max {:.1} ms",
        median(&mut waits),
        percentile(&mut waits, 100.0)
    );
}

/// Nearest-rank percentile.
fn percentile(values: &mut [f64], percent: f64) -> f64 {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let rank = (percent / 100.0 * values.len() as f64).ceil() as usize;
    values[rank.clamp(1, values.len()) - 1]
}

/// Search latency over typical queries, half of them misspelled. Reports each
/// query's median and the median and p95 over every timed query.
fn fuzzy(dir: PathBuf, data_root: PathBuf, runs: usize) {
    const QUERIES: &[&str] = &[
        "invoice",
        "report 004",
        "budget-012345",
        "summary",
        "contract docx",
        "design-000150",
        "scrnshot",
        "invioce",
        "metting",
        "sumary",
        "resmue",
        "bakup",
    ];
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    runtime.block_on(async {
        let bench = kura::bench::SearchBench::new(data_root);
        let count = bench.scan(&dir).await.expect("scan");
        println!("index: {count} items");
        let mut all = Vec::new();
        for query in QUERIES {
            let mut times = Vec::new();
            let mut results = 0;
            for _ in 0..runs {
                let started = Instant::now();
                results = bench.query(query).await.expect("query");
                times.push(ms(started.elapsed()));
            }
            println!(
                "  query {:<16} {:>3} results  median {:>6.1} ms",
                format!("{query:?}"),
                results,
                median(&mut times.clone())
            );
            all.extend(times);
        }
        all.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p95 = all[((all.len() as f64 * 0.95).ceil() as usize).saturating_sub(1)];
        println!(
            "all {} queries x {runs} runs: median {:.1} ms  p95 {:.1} ms",
            QUERIES.len(),
            median(&mut all),
            p95
        );
    });
}

fn ms(duration: std::time::Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    values[values.len() / 2]
}
