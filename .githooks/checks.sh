#!/usr/bin/env bash
# Runs Kura's checks. Usage: .githooks/checks.sh [frontend|rust|all]
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
suite="${1:-all}"
frontend() {
  npm run --silent check:format
  npm run --silent typecheck
  npm run --silent test
}
rust() {
  cargo fmt --manifest-path src-tauri/Cargo.toml --all --check
  cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets -- -D warnings
  cargo test --manifest-path src-tauri/Cargo.toml --workspace
}
case "$suite" in
  frontend) frontend ;;
  rust) rust ;;
  all) frontend & f=$!; rust & r=$!; wait $f; wait $r ;;
  *) echo "Unknown suite: $suite" >&2; exit 2 ;;
esac
