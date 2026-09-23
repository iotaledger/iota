#!/usr/bin/env bash
#
# Turn the cargo output of check.sh into the findings in allowlist format: one
# line per distinct (file, matched type, variants or fields not covered), with
# the number of source sites that share it, sorted. Line numbers are dropped on
# purpose, so an edit that only moves code changes nothing, while
#   - a new variant on an enum changes the not-covered text of every site that
#     now leaves it to the wildcard (a listed name, or rustc's "and N more"),
#   - a new wildcard site adds a line or raises a count,
#   - a fixed site removes a line or lowers a count.
# Sites are deduplicated by exact location first, so a crate's lib and test
# targets, which print the same diagnostic for the same match, count once.
#
# The lint reports two shapes, both parsed: an enum `match` whose wildcard covers
# variants ("some variants are not matched explicitly", label "patterns ... not
# covered") and a struct pattern whose `..` covers fields ("some fields are not
# explicitly listed", label "fields ... not listed").
#
# Types from other crates print fully qualified because rustc_wrapper.sh passes
# -Ztrim-diagnostic-paths=no and -Zwrite-long-types-to-disk=no; the output is
# only stable under those flags.
#
# Exits 1 if any finding in the log could not be parsed into all three fields,
# so a change in rustc's wording fails the run instead of emptying the findings.
set -uo pipefail
# Byte order for sort/uniq, so the output is identical on any runner locale.
export LC_ALL=C

log="$1"

# One line per finding: "file:line:col<TAB>matched type<TAB>variants or fields not covered".
# The three lines of a finding, as rustc renders them (gutter first):
#      --> crates/x/src/y.rs:12:11
#       |           ^ patterns `T::A` and `T::B` not covered       (label; on the
#       |_______^ patterns ... not covered                          closing line of
#                                                                    a multi-line span)
#       = note: the matched value is of type `T` and the ...
# The label line is required to start with an empty gutter so a quoted source
# line ("12 |  // not covered") can never be taken for it.
awk '
/^warning: some (variants are not matched explicitly|fields are not explicitly listed)/ {
  b=1; s=""; t=""; v=""; blocks++; next
}
b && s=="" && /^ *--> / { s=$2 }
b && v=="" && /^ *\|[ _|]*\^* *(patterns?|fields?) `.* not (covered|listed)$/ {
  v=$0; sub(/^ *\|[ _|]*\^* *(patterns?|fields?) /,"",v); sub(/ not (covered|listed)$/,"",v)
}
b && /^ *= note: the (matched value|pattern) is of type `/ {
  t=$0; sub(/.*of type `/,"",t); sub(/`.*/,"",t)
  if (s=="" || v=="") { print "findings.sh: unparsed finding ending at log line " NR > "/dev/stderr"; bad=1 }
  print s "\t" t "\t" v; rows++; b=0
}
END {
  if (blocks != rows) { print "findings.sh: " blocks+0 " findings in the log, " rows+0 " parsed" > "/dev/stderr"; bad=1 }
  exit bad
}
' "$log" \
  | sort -u \
  | awk -F'\t' '{ f=$1; sub(/:[0-9]+:[0-9]+$/,"",f); print f " | " $2 " | " $3 }' \
  | sort | uniq -c \
  | awk '{c=$1; $1=""; sub(/^ /,""); printf "%s | sites=%d\n", $0, c}'
rc="${PIPESTATUS[*]}"
if [[ "$rc" =~ [1-9] ]]; then
  echo "findings.sh: pipeline failed (exit statuses: $rc)" >&2
  exit 1
fi
