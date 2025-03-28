#!/usr/bin/env bash
set -euo pipefail

# Define the GitHub repository and base URL for raw file access
REPO="iotaledger/new_supply"
RAW_BASE="https://raw.githubusercontent.com/$REPO/main"
# Name of the output JSON file where aggregated data will be stored
OUTPUT="mainnet_unlocks_aggregated.json"
# Create a temporary directory to store downloaded CSV files
TMP_DIR="$(mktemp -d)"

echo "Downloading summary.csv files from $REPO..."

# List of folders containing the CSV files in the repository
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

# Loop over each folder and download its summary.csv file
for folder in "${FOLDERS[@]}"; do
  # Build the URL for the CSV file within the folder
  url="$RAW_BASE/$folder/summary.csv"
  # Create a safe filename by replacing any slashes with underscores
  dest="$TMP_DIR/${folder//\//_}.csv"

  echo "Fetching $url"
  # Download the CSV file; exit on any error (-sSf ensures silent failure with error reporting)
  curl -sSf "$url" -o "$dest"
done

echo "Aggregating by exact unlock date string..."

# Declare an associative array to aggregate token values by their unlock date
declare -A locked_by_date
total_lines=0
valid_lines=0

# Process each downloaded CSV file
for file in "$TMP_DIR"/*.csv; do
  echo "Processing $file"
  # Count the number of lines in the file (excluding the header)
  line_count=$(tail -n +2 "$file" | wc -l | xargs)
  echo "Lines (excluding header): $line_count"

  # Read each line of the CSV (skipping the header)
  while IFS= read -r line; do
    let "total_lines+=1"

    # Split the CSV line into tokens (first field) and unlock_date (second field)
    tokens="${line%%,*}"
    unlock_date="${line#*,}"

    # Trim any extra whitespace from both values
    tokens="$(echo "$tokens" | xargs)"
    unlock_date="$(echo "$unlock_date" | tr -d '\r' | xargs)"

    # Validate that the token value is numeric; exit if not
    if ! [[ "$tokens" =~ ^[0-9]+$ ]]; then
      echo "Invalid token amount: '$tokens' → Aborting." >&2
      exit 1
    fi

    # Convert tokens to nano-units (assuming 1 token = 1000 nano-units)
    nanos=$((tokens * 1000))
    let "valid_lines+=1"

    # Sum the tokens for the same unlock date (if the key already exists, add to it)
    current="${locked_by_date[$unlock_date]:-0}"
    locked_by_date["$unlock_date"]=$((current + nanos))
  done < <(tail -n +2 "$file")
done

echo "Total lines parsed: $total_lines"
echo "Valid unlock entries: $valid_lines"
echo "Unique timestamps: ${#locked_by_date[@]}"

# If no valid data was aggregated, output an empty JSON array and exit
if [[ ${#locked_by_date[@]} -eq 0 ]]; then
  echo "No data found – writing empty JSON."
  echo "[]" > "$OUTPUT"
  exit 0
fi

echo "Writing $OUTPUT ..."

# Create a temporary file for building the JSON output
tmp_json=$(mktemp)

# --- Calculation of "Still Locked" Tokens ---
# We first compute the total locked tokens across all dates.
# Then, by sorting the unlock dates chronologically, we subtract the cumulative unlocked
# tokens at each point from the total to obtain the remaining locked amount.

# 1. Calculate the total locked tokens from all aggregated entries
total_locked=0
for ts in "${!locked_by_date[@]}"; do
  total_locked=$((total_locked + locked_by_date[$ts]))
done

# 2. Sort the unlock date strings in ascending order
IFS=$'\n' sorted_keys=($(printf "%s\n" "${!locked_by_date[@]}" | sort))

# 3. Build an associative array mapping each timestamp to the remaining locked tokens at that time
declare -A still_locked_by_date
cumulative_unlocked=0

for ts in "${sorted_keys[@]}"; do
  unlocked="${locked_by_date[$ts]}"
  # Accumulate the unlocked tokens as we move forward in time
  cumulative_unlocked=$((cumulative_unlocked + unlocked))
  # Calculate the remaining locked tokens at this timestamp
  still_locked=$((total_locked - cumulative_unlocked))
  still_locked_by_date["$ts"]=$still_locked
done

# Generate the JSON output from the computed data
{
  echo "["
  first=true

  # Iterate over the sorted timestamps to output each JSON object
  for ts in "${sorted_keys[@]}"; do
    amount="${still_locked_by_date[$ts]}"
    # Add a comma between objects (skip before the first element)
    [[ $first == true ]] && first=false || echo ","
    # Convert the original date string to ISO 8601 format
    iso_ts=$(echo "$ts" | sed -E 's/ ([+0-9]+ UTC)//' | sed 's/ /T/')Z
    printf '  { "timestamp": "%s", "amount_still_locked": %s }' "$iso_ts" "$amount"
  done

  echo
  echo "]"
} > "$tmp_json"

# Use jq to format the JSON output nicely
jq . "$tmp_json" > "$OUTPUT"
# Remove the temporary JSON file
rm -f "$tmp_json"

echo "Done: $OUTPUT"
