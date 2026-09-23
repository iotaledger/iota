#!/usr/bin/env bash
# Turn the cargo output of check.sh into a markdown report: SDK matches per site,
# split into production and test code, everything else as per-type counts.
set -uo pipefail
export LC_ALL=C

log="$1"
raw="$(mktemp "${TMPDIR:-/tmp}/nelint-raw.XXXXXX")"
sites="$(mktemp "${TMPDIR:-/tmp}/nelint-sites.XXXXXX")"
trap 'rm -f "$raw" "$sites"' EXIT

# "path:line:col<TAB>matched type" per finding; see findings.sh for the shapes.
awk '
/^warning: some (variants are not matched explicitly|fields are not explicitly listed)/ {b=1; p=""; next}
b && p=="" && /^ *--> / { p=$2 }
b && /^ *= note: the (matched value|pattern) is of type `/ { s=$0; sub(/.*of type `/,"",s); sub(/`.*/,"",s); print p "\t" s; b=0 }
' "$log" > "$raw"

# A crate's lib and test targets report the same site twice.
sort -u "$raw" > "$sites"

n_raw=$(wc -l < "$raw" | tr -d ' ')
n_sites=$(wc -l < "$sites" | tr -d ' ')
n_crates=$(cut -f1 "$sites" | sed -E 's#(crates/[^/]+|iota-execution/[^/]+/[^/]+)/.*#\1#' | sort -u | wc -l | tr -d ' ')

# Types from other crates are fully qualified (see the flags in rustc_wrapper.sh).
SDK_RE='iota_sdk_types|iota_sdk_grpc'
TEST_RE='/(tests|unit_tests|examples|benches)/|_tests?\.rs:'

row() { awk -F'\t' '{ printf "| `%s` | `%s` |\n", $1, $2 }'; }
table_hdr() { echo "| location | matched type |"; echo "|---|---|"; }

sdk_prod=$(grep -E "$SDK_RE" "$sites" | grep -vE "$TEST_RE" || true)
sdk_test=$(grep -E "$SDK_RE" "$sites" | grep -E "$TEST_RE" || true)

echo "## Non-exhaustive SDK enum matches"
echo
echo "**$n_sites** source sites match a \`#[non_exhaustive]\` enum with a wildcard arm, across **$n_crates** crates ($n_raw raw findings before collapsing lib/test duplicates). Warn-level: a finding by itself never fails the job; a finding missing from the committed allowlist.txt does (see the Allowlist section below)."
echo
echo "### SDK matches in production code, review these first"
echo
if [[ -z "$sdk_prod" ]]; then echo "_none_"; else table_hdr; printf '%s\n' "$sdk_prod" | row; fi
echo
echo "### SDK matches in tests / examples"
echo
if [[ -z "$sdk_test" ]]; then echo "_none_"; else table_hdr; printf '%s\n' "$sdk_test" | row; fi
echo
echo "### Other non_exhaustive matches (internal and third-party enums, informational)"
echo
echo "| matched type | sites |"
echo "|---|---|"
{ grep -vE "$SDK_RE" "$sites" || true; } | cut -f2 | sort | uniq -c | sort -rn \
  | awk '{c=$1; $1=""; sub(/^ /,""); printf "| `%s` | %d |\n", $0, c}'
