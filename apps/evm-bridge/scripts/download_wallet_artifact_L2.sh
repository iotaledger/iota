#!/bin/bash

set -e

OUTPUT_DIR="wallet-dist-L2"

# URL of the MetaMask releases page
RELEASES_URL="https://github.com/MetaMask/metamask-extension/releases"

# Fetch the latest release page
LATEST_RELEASE_PAGE=$(curl -s $RELEASES_URL | grep -o 'href="/MetaMask/metamask-extension/releases/tag/[^"]*"' | sed 's/href="//;s/"$//' | head -n 1)

# Extract the latest release version
LATEST_VERSION=$(echo $LATEST_RELEASE_PAGE | grep -Eo 'v[0-9]+\.[0-9]+\.[0-9]+' | sed 's/v//')

# Construct the download URL for the MetaMask Chrome zip file
DOWNLOAD_URL="https://github.com/MetaMask/metamask-extension/releases/download/v$LATEST_VERSION/metamask-chrome-$LATEST_VERSION.zip"

mkdir -p "$OUTPUT_DIR"
TEMP_FILE=$(mktemp)

if [ -d "$OUTPUT_DIR" ]; then
    rm -rf "$FOLDER_PATH"
fi

echo "Downloading artifact to $OUTPUT_DIR from $DOWNLOAD_URL"

# Download the artifact
if curl -L -o $TEMP_FILE $DOWNLOAD_URL; then

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
