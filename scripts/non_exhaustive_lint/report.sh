#!/usr/bin/env bash
#
# Turn the cargo output of check.sh into a readable markdown report.
#
# - One row per source site. `--all-targets` compiles a crate's lib and test
#   targets separately, so the same match is otherwise reported twice.
# - SDK matches first, split into production code and tests/examples, since a
#   wildcard hiding a new SDK variant in shipped code is what this check exists
#   to surface.
# - Every other non_exhaustive match (our own internal enums, third-party
#   crates) collapsed into per-type counts, informational only.
#
# Classification is by crate path, not by a list of names: rustc_wrapper.sh passes
# -Ztrim-diagnostic-paths=no and -Zwrite-long-types-to-disk=no, so every type
# from another crate carries that crate in the diagnostic, and a match is an SDK
# match iff its type mentions iota_sdk_types or iota_sdk_grpc.
set -uo pipefail
export LC_ALL=C

log="$1"
raw="$(mktemp "${TMPDIR:-/tmp}/nelint-raw.XXXXXX")"
sites="$(mktemp "${TMPDIR:-/tmp}/nelint-sites.XXXXXX")"
trap 'rm -f "$raw" "$sites"' EXIT

# One line per finding (enum match or struct pattern, see findings.sh):
# "path:line:col<TAB>matched type".
awk '
/^warning: some (variants are not matched explicitly|fields are not explicitly listed)/ {b=1; p=""; next}
b && p=="" && /^ *--> / { p=$2 }
b && /^ *= note: the (matched value|pattern) is of type `/ { s=$0; sub(/.*of type `/,"",s); sub(/`.*/,"",s); print p "\t" s; b=0 }
' "$log" > "$raw"

# Collapse to one row per site, keeping the most qualified type name should a
# future rustc ever trim one of the two targets' diagnostics again.
awk -F'\t' '
{ t=$2; s=0; if (t ~ /iota_sdk/) s+=1000; s+=gsub(/::/,"::",t);
  if (!($1 in best) || s>score[$1]) { best[$1]=$2; score[$1]=s } }
END { for (k in best) print k "\t" best[k] }
' "$raw" | sort > "$sites"

n_raw=$(wc -l < "$raw" | tr -d ' ')
n_sites=$(wc -l < "$sites" | tr -d ' ')
n_crates=$(cut -f1 "$sites" | sed -E 's#(crates/[^/]+|iota-execution/[^/]+/[^/]+)/.*#\1#' | sort -u | wc -l | tr -d ' ')

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
