#!/usr/bin/env bash
set -euo pipefail

REPO="iotaledger/new_supply"
RAW_BASE="https://raw.githubusercontent.com/$REPO/main"
OUTPUT="mainnet_unlocks.json"
TMP_DIR="$(mktemp -d)"

echo "Downloading summary.csv files from $REPO..."

FOLDERS=(
  "Assembly_IF_Members"
  "Assembly_Investors"
  "IOTA_Airdrop"
  "IOTA_Foundation"
  "New_Investors"
  "TEA"
  "Treasury_DAO"
  "UAE"
)

for folder in "${FOLDERS[@]}"; do
  url="$RAW_BASE/$folder/summary.csv"
  dest="$TMP_DIR/${folder//\//_}.csv"

  echo "➡ Fetching $url"
  curl -sSf "$url" -o "$dest"
done

echo "Aggregating by exact unlock date string..."

declare -A locked_by_date
total_lines=0
valid_lines=0

for file in "$TMP_DIR"/*.csv; do
  echo "🔍 Processing $file"
  line_count=$(tail -n +2 "$file" | wc -l | xargs)
  echo "📄 Lines (excluding header): $line_count"

  while IFS= read -r line; do
    # echo "Raw line: $line"
    let "total_lines+=1"

    tokens="${line%%,*}"
    unlock_date="${line#*,}"

    tokens="$(echo "$tokens" | xargs)"
    unlock_date="$(echo "$unlock_date" | tr -d '\r' | xargs)"

    if ! [[ "$tokens" =~ ^[0-9]+$ ]]; then
      echo "Invalid token amount: '$tokens' → Aborting." >&2
      exit 1
    fi

    nanos=$((tokens * 1000))
    let "valid_lines+=1"

    # echo "Parsed: [$tokens] → [$unlock_date]"

    current="${locked_by_date[$unlock_date]:-0}"
    locked_by_date["$unlock_date"]=$((current + nanos))
  done < <(tail -n +2 "$file")
done

echo "Total lines parsed: $total_lines"
echo "Valid unlock entries: $valid_lines"
echo "Unique timestamps: ${#locked_by_date[@]}"

if [[ ${#locked_by_date[@]} -eq 0 ]]; then
  echo "No data found – writing empty JSON."
  echo "[]" > "$OUTPUT"
  exit 0
fi

echo "Writing $OUTPUT ..."

tmp_json=$(mktemp)

# Cumulative locked tokens

# Sort timestamps in descending order for cumulative calculation
IFS=$'\n' sorted_keys=($(printf "%s\n" "${!locked_by_date[@]}" | sort -r))

# Build cumulative map
declare -A cumulative_by_date
running_total=0

for ts in "${sorted_keys[@]}"; do
  amount="${locked_by_date[$ts]}"
  running_total=$((running_total + amount))
  cumulative_by_date["$ts"]=$running_total
done

# Sort ascending for final JSON output
IFS=$'\n' sorted_keys=($(printf "%s\n" "${!cumulative_by_date[@]}" | sort))

# Output JSON
{
  echo "["
  first=true

  for ts in "${sorted_keys[@]}"; do
    amount="${cumulative_by_date[$ts]}"
    [[ $first == true ]] && first=false || echo ","
    iso_ts=$(echo "$ts" | sed -E 's/ ([+0-9]+ UTC)//' | sed 's/ /T/')Z
    printf '  { "timestamp": "%s", "amount_locked": %s }' "$iso_ts" "$amount"
  done

  echo
  echo "]"
} > "$tmp_json"

# Beautify via jq
jq . "$tmp_json" > "$OUTPUT"
rm -f "$tmp_json"

echo "Done: $OUTPUT"
