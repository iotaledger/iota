#!/usr/bin/env bash
# Rebuilds the Move package fixtures under trace-adapter/tests/ by running
# `iota move test --trace-execution` in every test package.
#
# Usage:
#   ./rebuild-fixtures.sh                # build the iota binary (with the
#                                         # `tracing` feature) then rebuild
#                                         # every test package
#   ./rebuild-fixtures.sh --skip-build    # reuse an already-built binary
#
# A test failure reported for a given package is not necessarily a problem:
# several fixtures (abort_assert, abort_native, macro_abort, ...) intentionally
# trigger a Move abort to test debugger behavior and have no
# #[expected_failure] annotation. The build/ and traces/ files are written
# regardless of the unit test's pass/fail verdict, so this script only
# treats a package as failed if the Move test framework never got to the
# point of running tests (e.g. a compile error or a missing `tracing`
# feature on the binary).
set -uo pipefail

repo_root="$(git rev-parse --show-toplevel)"
tests_dir="$repo_root/external-crates/move/crates/move-analyzer/trace-adapter/tests"
iota_bin="${IOTA_BIN:-$repo_root/target/debug/iota}"
log_dir="$(mktemp -d)"

skip_build=0
if [[ "${1:-}" == "--skip-build" ]]; then
  skip_build=1
fi

if [[ $skip_build -eq 0 ]]; then
  echo "==> Building iota binary with the tracing feature..."
  (cd "$repo_root" && cargo build -p iota --features tracing)
fi

if [[ ! -x "$iota_bin" ]]; then
  echo "error: $iota_bin not found or not executable." >&2
  echo "Build it with: cargo build -p iota --features tracing" >&2
  exit 1
fi

ok=()
aborted=()
skipped=()
errored=()

for dir in "$tests_dir"/*/; do
  name="$(basename "$dir")"
  manifest="$dir/Move.toml"

  if [[ ! -f "$manifest" ]]; then
    skipped+=("$name")
    continue
  fi

  echo "==> Rebuilding $name"
  log_file="$log_dir/$name.log"
  if (cd "$dir" && "$iota_bin" move test --trace-execution) >"$log_file" 2>&1; then
    ok+=("$name")
  elif grep -q "Running Move unit tests" "$log_file"; then
    # Build + trace generation succeeded; the test itself aborted as designed.
    aborted+=("$name")
  else
    errored+=("$name")
  fi
done

echo
echo "==== Summary ===="
echo "Rebuilt cleanly (${#ok[@]}): ${ok[*]:-none}"
echo "Rebuilt, unit test aborted as designed (${#aborted[@]}): ${aborted[*]:-none}"
echo "Skipped, no Move.toml (${#skipped[@]}): ${skipped[*]:-none}"
if [[ ${#errored[@]} -gt 0 ]]; then
  echo "FAILED to rebuild (${#errored[@]}): ${errored[*]}"
  for name in "${errored[@]}"; do
    echo "  --- $name ($log_dir/$name.log) ---"
  done
  exit 1
fi

echo "All fixtures rebuilt. Logs in $log_dir"
