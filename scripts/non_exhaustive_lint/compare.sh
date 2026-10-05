#!/usr/bin/env bash
# Compare the findings of findings.sh with an allowlist; exit 1 if they differ.
# Usage: compare.sh <allowlist> <findings>
set -uo pipefail
export LC_ALL=C

allowlist="$1"
findings_in="$2"

if [[ ! -f "$allowlist" ]]; then
  echo "ERROR: no allowlist at $allowlist. Commit one (comment lines only for an empty allowlist, or \`scripts/non_exhaustive_lint/check.sh --allow-all\` to allow every current finding)." >&2
  exit 1
fi

allowed="$(mktemp "${TMPDIR:-/tmp}/nelint-allowed.XXXXXX")"
findings="$(mktemp "${TMPDIR:-/tmp}/nelint-sorted.XXXXXX")"
trap 'rm -f "$allowed" "$findings"' EXIT

# Keys never contain "#", so a trailing "# reason" can be stripped.
grep -v '^#' "$allowlist" | sed -E 's/[[:space:]]+#.*$//; s/[[:space:]]+$//' \
  | grep -v '^$' | sort > "$allowed"
sort "$findings_in" > "$findings"
n_allowed=$(wc -l < "$allowed")
n_findings=$(wc -l < "$findings")

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
  "+": a finding not in the allowlist. Handle the variants rustc names for that match in the log,
  or allow it by adding the exact line to scripts/non_exhaustive_lint/allowlist.txt (optionally followed by
  "# reason"). "-": an allowlist entry no longer found; remove it. A "-"/"+" pair that differs
  only in sites=N means a wildcard was added or removed on that type in that file; the log lists every site.
EOF
if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
  {
    printf '\n## Findings not in the allowlist\n\n'
    printf '`+` is a finding not in `scripts/non_exhaustive_lint/allowlist.txt`: handle the variants rustc names for that match in the log, or allow it by adding the exact line (optionally followed by `# reason`). `-` is an allowlist entry no longer found: remove it. A `-`/`+` pair that differs only in `sites=N` means a wildcard was added or removed on that type in that file; the log lists every site.\n\n'
    printf '```diff\n%s\n```\n' "$diff_out"
  } >> "$GITHUB_STEP_SUMMARY"
fi
exit 1
