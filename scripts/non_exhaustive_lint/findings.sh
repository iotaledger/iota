#!/usr/bin/env bash
# Reduce the cargo output of check.sh to one line per distinct
# (file, matched type, variants or fields not covered) with its site count.
set -uo pipefail
export LC_ALL=C

log="$1"

# rustc renders a finding as (gutter first):
#      --> crates/x/src/y.rs:12:11
#       |           ^ patterns `T::A` and `T::B` not covered        single-line span
#       |_______^ patterns ... not covered                           multi-line span
#       = note: the matched value is of type `T` and the ...
# or, for a struct pattern, "some fields are not explicitly listed" / "fields ...
# not listed" / "the pattern is of type". The label must start with an empty
# gutter so a quoted source line ("12 |  // not covered") is never taken for it.
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
