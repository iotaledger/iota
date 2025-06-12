#!/bin/bash

set -e

OWNER="iotaledger"
REPO="iota"
OUTPUT_DIR="wallet-dist-L1"

if [ -z "$GITHUB_TOKEN" ]; then
    echo "Error: GITHUB_TOKEN environment variable is required"
    exit 1
fi

if ! command -v jq &> /dev/null; then
    echo "Error: jq is required but not installed."
    exit 1
fi

mkdir -p "$OUTPUT_DIR"

# Get the latest successful artifact ID from the workflow
echo "Finding latest wallet production build artifact..."
ARTIFACT_ID=$(curl -s -H "Authorization: token $GITHUB_TOKEN" \
     -H "Accept: application/vnd.github.v3+json" \
     "https://api.github.com/repos/$OWNER/$REPO/actions/workflows/apps_wallet_prod_build.yml/runs?status=success&per_page=1" | \
jq -r '.workflow_runs[0].artifacts_url' | \
xargs curl -s -H "Authorization: token $GITHUB_TOKEN" \
     -H "Accept: application/vnd.github.v3+json" | \
jq -r '.artifacts[0].id')

if [ -z "$ARTIFACT_ID" ] || [ "$ARTIFACT_ID" = "null" ]; then
    echo "Error: No artifacts found"
    exit 1
fi

echo "Downloading artifact $ARTIFACT_ID to $OUTPUT_DIR..."

TEMP_FILE=$(mktemp)

# Download the artifact
if curl -L -s -H "Authorization: token $GITHUB_TOKEN" \
        -H "Accept: application/vnd.github.v3+json" \
        "https://api.github.com/repos/$OWNER/$REPO/actions/artifacts/$ARTIFACT_ID/zip" \
        --output "$TEMP_FILE"; then

    # Extract the zip file
    echo "Extracting artifact..."
    unzip -q -o "$TEMP_FILE" -d "$OUTPUT_DIR"

    # Clean up
    rm "$TEMP_FILE"
    echo "Successfully downloaded and extracted artifact to $OUTPUT_DIR"
else
    echo "Error: Failed to download artifact"
    rm "$TEMP_FILE"
    exit 1
fi
