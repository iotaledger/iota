#!/bin/bash

set -e

OUTPUT_DIR="wallet-dist-L2"

VERSION="${METAMASK_VERSION}"
echo "Using MetaMask version: $VERSION"

# Construct the download URL for the MetaMask Chrome zip file
DOWNLOAD_URL="https://github.com/MetaMask/metamask-extension/releases/download/v$VERSION/metamask-chrome-$VERSION.zip"

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
