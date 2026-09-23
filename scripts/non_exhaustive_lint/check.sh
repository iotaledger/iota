#!/usr/bin/env bash
#
# Gate on iota-sdk-types #[non_exhaustive] enum variants that a `match` leaves
# to a wildcard arm. A new SDK variant would otherwise land silently in that arm.
# (The lint also reports `#[non_exhaustive]` struct fields left to `..`; those
# are kept. It does not fire on `if let`, `let else` or `matches!`; see
# README.md, coverage gaps.)
#
# The lint runs at warn level and a finding by itself never fails cargo. What
# fails the job is a finding that is not in the committed allowlist.txt, or an
# allowlist entry that is no longer found. Findings are reduced to line-number-
# free keys (findings.sh), so moving code changes nothing, while a new variant
# left to a wildcard, a new wildcard site, or a fixed site all show up in the
# diff. To allow a finding, add its exact line from the diff to allowlist.txt
# (a trailing "# reason" is allowed); to allow everything currently reported, run
#
#   scripts/non_exhaustive_lint/check.sh --allow-all
#
# and commit allowlist.txt.
#
# Exit status: 0 when the findings match the allowlist (or --allow-all wrote
# it); 2 on a usage error; cargo's own status when `cargo check` fails; 1 for
# every other failure, each with its own ERROR line: the scope could not be
# derived, the self-test found no finding at all (the lint is no longer being
# applied), report.sh or findings.sh failed, no allowlist is committed, or the
# findings differ from the allowlist. Do not run this under `-D warnings`;
# findings are warnings by contract.
#
# Coverage is derived, not curated: every workspace crate that can reach
# iota-sdk-types in its dependency tree is in scope, i.e. every crate that could
# name an SDK type (directly, or through a re-export from another crate). Crates
# with no path to iota-sdk-types are excluded, so their unrelated non_exhaustive
# enums stay out. See README.md for what is and is not covered.
#
# No crate source is edited. The unstable, nightly-only lint
# (non_exhaustive_omitted_patterns) is injected through a RUSTC_WORKSPACE_WRAPPER
# that appends the crate attributes only when the crate being compiled is in
# scope (keyed on CARGO_PKG_NAME). The wrapper runs for workspace members only,
# and keying on the crate's own identity means a dependency of an in-scope crate
# is never injected, so third-party enums in the dependency graph do not trip it.
#
# report.sh turns the raw compiler output into a readable report: printed to
# stdout, appended to the GitHub job summary when GITHUB_STEP_SUMMARY is set, and
# written with findings.txt, the in-scope crate list and the full cargo log to
# NELINT_OUT_DIR when that is set, for upload as a CI artifact.
set -uo pipefail
# One collation for sort/comm/uniq here and in findings.sh, whatever the runner's
# locale; comm silently drops lines when its inputs were sorted under another one.
export LC_ALL=C
# No ANSI escapes in anything the scripts parse (cargo tree, the check log).
export CARGO_TERM_COLOR=never

mode="${1:-}"
case "$mode" in
  ""|--allow-all) ;;
  *) echo "usage: $0 [--allow-all]" >&2; exit 2 ;;
esac

# Resolve sibling paths from $0 before changing directory: $0 may be relative to
# the caller's cwd, which the `cd` below would otherwise invalidate.
here="$(cd "$(dirname "$0")" && pwd)"
WRAPPER="${WRAPPER:-$here/rustc_wrapper.sh}"
ALLOWLIST="$here/allowlist.txt"

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

NIGHTLY="${NIGHTLY:-nightly-2026-06-29}"

pkgs_file="$(mktemp "${TMPDIR:-/tmp}/nelint-pkgs.XXXXXX")"
members_file="$(mktemp "${TMPDIR:-/tmp}/nelint-members.XXXXXX")"
log_file="$(mktemp "${TMPDIR:-/tmp}/nelint-out.XXXXXX")"
findings="$(mktemp "${TMPDIR:-/tmp}/nelint-findings.XXXXXX")"
allowed="$(mktemp "${TMPDIR:-/tmp}/nelint-allowed.XXXXXX")"
trap 'rm -f "$pkgs_file" "$members_file" "$log_file" "$findings" "$allowed"' EXIT

# In scope: workspace members that reach iota-sdk-types in their dependency tree
# (any edge kind, so lib/bin/test/build code is all covered). `cargo tree --invert`
# lists everything that depends on iota-sdk-types, transitively; --workspace so
# that is every member, not only default members; intersecting with the
# workspace member names drops out-of-workspace crates. --all-features, as the
# check below passes it, so an optional dependency that reaches iota-sdk-types
# only behind a feature is in scope too.
cargo metadata --no-deps --format-version 1 \
  | jq -r '.packages[].name' | sort -u > "$members_file"
members_rc="${PIPESTATUS[*]}"
cargo tree --workspace --invert iota-sdk-types --edges all --all-features --prefix none \
  | sed -E 's/ v[0-9].*//' | sort -u \
  | comm -12 "$members_file" - > "$pkgs_file"
scope_rc="${PIPESTATUS[*]}"

# A failure anywhere in the two pipelines, or an empty result, means `cargo
# metadata`/`cargo tree`/jq/comm failed or the dependency was renamed away, not
# that there is nothing to lint. Fail with the real cause rather than linting a
# truncated scope or letting the self-test below misreport it as "lint not applied".
if [[ "$members_rc $scope_rc" =~ [1-9] ]]; then
  echo "ERROR: deriving the in-scope crates failed (exit statuses: members '$members_rc', scope '$scope_rc')." >&2
  exit 1
fi
if [[ ! -s "$pkgs_file" ]]; then
  echo "ERROR: found no workspace crates depending on iota-sdk-types (cargo metadata/tree or jq failed?)." >&2
  exit 1
fi
echo "non_exhaustive_omitted_patterns: $(wc -l < "$pkgs_file") crates in scope"

# --all-features so a match placed behind a `#[cfg(feature = "...")]` that is off
# in the default build is still compiled and linted; otherwise such a match would
# be silently invisible forever.
RUSTC_WORKSPACE_WRAPPER="$WRAPPER" \
NELINT_PKGS_FILE="$pkgs_file" \
NELINT_LEVEL=warn \
  cargo +"$NIGHTLY" check --workspace --all-targets --all-features 2>&1 | tee "$log_file"
cargo_rc=${PIPESTATUS[0]}

# The compiler output and the scope go into the artifact even when the run fails
# below; the report and the findings follow once they exist.
if [[ -n "${NELINT_OUT_DIR:-}" ]]; then
  mkdir -p "$NELINT_OUT_DIR"
  cp "$log_file" "$NELINT_OUT_DIR/cargo-check.log"
  cp "$pkgs_file" "$NELINT_OUT_DIR/in-scope-crates.txt"
fi

# The lint runs at warn and never fails cargo, so a non-zero exit is a real build
# error (broken code, missing toolchain, ...), not a finding. The explicit check
# is needed because the script runs with pipefail but without -e.
if [[ "$cargo_rc" -ne 0 ]]; then
  echo "ERROR: cargo check failed (exit $cargo_rc); this is a build error, not a lint finding." >&2
  exit "$cargo_rc"
fi

# Self-test: prove the lint was actually injected, independent of how clean the
# codebase happens to be. We assert the run produced at least one finding, by
# the note every finding carries (not the bare lint name, which an "unknown lint"
# warning would also print). In-scope crates match third-party non_exhaustive
# enums (object_store, ...) as well as SDK ones, so a working injection
# essentially always yields at least one finding; zero means the wrapper is not
# being applied. This runs before the allowlist diff so a dead wrapper is
# reported as such, not as "every allowlist entry disappeared".
if ! grep -qF 'and the `non_exhaustive_omitted_patterns` attribute was found' "$log_file"; then
  echo "ERROR: self-test failed: no non_exhaustive_omitted_patterns findings; the lint is not being applied." >&2
  exit 1
fi

if ! report="$("$here/report.sh" "$log_file")"; then
  echo "ERROR: report.sh failed." >&2
  exit 1
fi
printf '%s\n' "$report"
if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
  printf '%s\n' "$report" >> "$GITHUB_STEP_SUMMARY"
fi

# findings.sh exits non-zero when a finding does not parse; a parse regression
# must fail the run, not empty the findings (an empty result is the same case).
if ! "$here/findings.sh" "$log_file" > "$findings"; then
  echo "ERROR: findings.sh could not parse the findings (see above)." >&2
  exit 1
fi
n_findings=$(wc -l < "$findings")
if [[ "$n_findings" -eq 0 ]]; then
  echo "ERROR: the log has findings but findings.sh produced none." >&2
  exit 1
fi

if [[ -n "${NELINT_OUT_DIR:-}" ]]; then
  printf '%s\n' "$report" > "$NELINT_OUT_DIR/report.md"
  cp "$findings" "$NELINT_OUT_DIR/findings.txt"
fi

if [[ "$mode" == "--allow-all" ]]; then
  {
    echo "# non_exhaustive_omitted_patterns findings allowed as known. Checked by scripts/non_exhaustive_lint/check.sh."
    echo "# Allow one finding by adding its exact line from the CI diff (any order, optionally followed by \`# reason\`);"
    echo "# allow all current findings with \`scripts/non_exhaustive_lint/check.sh --allow-all\` (drops the reasons)."
    echo "# Remove lines the check reports as no longer found."
    echo "# file | matched type | variants or fields not covered, as rustc lists them | sites=<number of match sites>"
    cat "$findings"
  } > "$ALLOWLIST" || { echo "ERROR: could not write $ALLOWLIST." >&2; exit 1; }
  echo "allowlist.txt written with all $n_findings current findings. Review it and commit it."
  exit 0
fi

if [[ ! -f "$ALLOWLIST" ]]; then
  echo "ERROR: no allowlist at $ALLOWLIST. Commit one (comment lines only for an empty allowlist, or \`$0 --allow-all\` to allow every current finding)." >&2
  exit 1
fi

# Compare key lines only, both sides sorted, so hand-added lines may sit anywhere.
# A trailing "# reason" on an allowlist line is dropped (keys never contain "#").
grep -v '^#' "$ALLOWLIST" | sed -E 's/[[:space:]]+#.*$//; s/[[:space:]]+$//' \
  | grep -v '^$' | sort > "$allowed"
n_allowed=$(wc -l < "$allowed")

if diff_out="$(diff -u --label allowlist.txt --label findings "$allowed" "$findings")"; then
  echo "findings match the allowlist ($n_findings entries)"
  if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
    printf '\n## Allowlist\n\nAll %s findings are in the allowlist.\n' "$n_findings" >> "$GITHUB_STEP_SUMMARY"
  fi
  exit 0
fi

printf '%s\n' "$diff_out"
cat >&2 <<EOF
ERROR: $n_findings findings, $n_allowed allowed; they differ (diff above).
  A "+" line is a finding that is not in the allowlist: a wildcard arm covers the listed
  variants (a changed "variants not covered" text means an enum gained a variant that a
  wildcard now swallows; a higher sites count is a new wildcard match). Handle it at that
  match, or allow it by adding the exact line to scripts/non_exhaustive_lint/allowlist.txt
  (optionally followed by "# reason"). A "-" line is an allowlist entry no longer found: remove it.
EOF
if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
  {
    printf '\n## Findings not in the allowlist\n\n'
    printf '`+` is a finding not in `scripts/non_exhaustive_lint/allowlist.txt`: handle it at that match, or allow it by adding the exact line (optionally followed by `# reason`). `-` is an allowlist entry no longer found: remove it.\n\n'
    printf '```diff\n%s\n```\n' "$diff_out"
  } >> "$GITHUB_STEP_SUMMARY"
fi
exit 1
