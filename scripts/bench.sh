#!/usr/bin/env bash
# Times Kura's directory listing and search on a generated fixture.
# Usage: scripts/bench.sh [file-count]   (default 100000)
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
count="${1:-100000}"
work="$(mktemp -d "${TMPDIR:-/tmp}/kura-bench.XXXXXX")"
trap 'rm -rf "$work"' EXIT

cargo build --quiet --release --manifest-path src-tauri/Cargo.toml --example bench
bench=src-tauri/target/release/examples/bench

"$bench" fixture "$work/files" "$count"
"$bench" list "$work/files" 7

# Webview side of the IPC hop: parsing the listing JSON.
"$bench" dump "$work/files" "$work/listing.json"
node -e '
const fs = require("fs");
const text = fs.readFileSync(process.argv[1], "utf8");
const times = [];
for (let i = 0; i < 5; i++) {
  const started = performance.now();
  JSON.parse(text);
  times.push(performance.now() - started);
}
times.sort((a, b) => a - b);
console.log(`  JSON.parse median ${times[2].toFixed(1)} ms (Node; the webview is similar)`);
' "$work/listing.json"

"$bench" search "$work/files" "$work/data"
