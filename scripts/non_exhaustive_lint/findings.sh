#!/usr/bin/env bash
# Reduce the cargo output of check.sh to one line per (file, matched type) with
# the number of match sites.
set -uo pipefail
export LC_ALL=C

log="$1"

# A finding's own `-->` line is the first in its block; the `--> <crate attribute>`
# of the "lint level is defined here" note comes after the type note.
awk '
/^warning: some (variants are not matched explicitly|fields are not explicitly listed)/ {
  b=1; s=""; blocks++; next
}
b && s=="" && /^ *--> / { s=$2 }
b && /^ *= note: the (matched value|pattern) is of type `/ {
  t=$0; sub(/.*of type `/,"",t); sub(/`.*/,"",t)
  if (s=="") { print "findings.sh: unparsed finding ending at log line " NR > "/dev/stderr"; bad=1 }
  print s "\t" t; rows++; b=0
}
END {
  if (blocks != rows) { print "findings.sh: " blocks+0 " findings in the log, " rows+0 " parsed" > "/dev/stderr"; bad=1 }
  exit bad
}
' "$log" \
  | sort -u \
  | awk -F'\t' '{ f=$1; sub(/:[0-9]+:[0-9]+$/,"",f); print f " | " $2 }' \
  | sort | uniq -c \
  | awk '{c=$1; $1=""; sub(/^ /,""); printf "%s | sites=%d\n", $0, c}'
rc="${PIPESTATUS[*]}"
if [[ "$rc" =~ [1-9] ]]; then
  echo "findings.sh: pipeline failed (exit statuses: $rc)" >&2
  exit 1
fi
