#!/usr/bin/env bash
# Nightly gate on iota-sdk-types #[non_exhaustive] enum variants that a `match`
# leaves to a wildcard arm: the findings of the rustc lint
# non_exhaustive_omitted_patterns, injected by rustc_wrapper.sh into every
# workspace crate whose dependency tree reaches iota-sdk-types, are compared
# with allowlist.txt.
#
# Not covered, so a green run says nothing about:
# - `if let`, `let else`, `while let` and `matches!` over an SDK enum: the lint
#   checks `match` wildcards only, and a future variant takes their else side.
# - Code behind cfg(not(feature = ..)) (the run uses --all-features), cfg(msim),
#   or a target cfg the runner does not match; external-crates/move; doctests.
# - rustc names at most three uncovered variants plus "and N more", so adding one
#   variant and removing another beyond the third leaves the key unchanged, and
#   replacing one wildcard site by another with the same key in the same file
#   keeps sites=N.
set -uo pipefail
# comm and sort must agree on collation; comm silently drops lines otherwise.
export LC_ALL=C
export CARGO_TERM_COLOR=never

mode="${1:-}"
case "$mode" in
  ""|--allow-all) ;;
  *) echo "usage: $0 [--allow-all]" >&2; exit 2 ;;
esac

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

cargo metadata --no-deps --format-version 1 \
  | jq -r '.packages[].name' | sort -u > "$members_file"
members_rc="${PIPESTATUS[*]}"
cargo tree --workspace --invert iota-sdk-types --edges all --all-features --prefix none \
  | sed -E 's/ v[0-9].*//' | sort -u \
  | comm -12 "$members_file" - > "$pkgs_file"
scope_rc="${PIPESTATUS[*]}"

if [[ "$members_rc $scope_rc" =~ [1-9] ]]; then
  echo "ERROR: deriving the in-scope crates failed (exit statuses: members '$members_rc', scope '$scope_rc')." >&2
  exit 1
fi
if [[ ! -s "$pkgs_file" ]]; then
  echo "ERROR: found no workspace crates depending on iota-sdk-types (cargo metadata/tree or jq failed?)." >&2
  exit 1
fi
echo "non_exhaustive_omitted_patterns: $(wc -l < "$pkgs_file") crates in scope"

RUSTC_WORKSPACE_WRAPPER="$WRAPPER" \
NELINT_PKGS_FILE="$pkgs_file" \
  cargo +"$NIGHTLY" check --workspace --all-targets --all-features 2>&1 | tee "$log_file"
cargo_rc=${PIPESTATUS[0]}

if [[ "$cargo_rc" -ne 0 ]]; then
  echo "ERROR: cargo check failed (exit $cargo_rc); this is a build error, not a lint finding." >&2
  exit "$cargo_rc"
fi

# Self-test on the note every finding carries, not on the lint name, which an
# "unknown lint" warning would also print. Before the allowlist diff, so a dead
# wrapper is not reported as "every allowlist entry disappeared".
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

if ! "$here/findings.sh" "$log_file" > "$findings"; then
  echo "ERROR: findings.sh could not parse the findings (see above)." >&2
  exit 1
fi
n_findings=$(wc -l < "$findings")
if [[ "$n_findings" -eq 0 ]]; then
  echo "ERROR: the log has findings but findings.sh produced none." >&2
  exit 1
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

# Keys never contain "#", so a trailing "# reason" can be stripped.
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
  "+": a finding not in the allowlist. Handle the listed variants at that match, or allow it by
  adding the exact line to scripts/non_exhaustive_lint/allowlist.txt (optionally followed by
  "# reason"). "-": an allowlist entry no longer found; remove it.
EOF
if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
  {
    printf '\n## Findings not in the allowlist\n\n'
    printf '`+` is a finding not in `scripts/non_exhaustive_lint/allowlist.txt`: handle it at that match, or allow it by adding the exact line (optionally followed by `# reason`). `-` is an allowlist entry no longer found: remove it.\n\n'
    printf '```diff\n%s\n```\n' "$diff_out"
  } >> "$GITHUB_STEP_SUMMARY"
fi
exit 1
