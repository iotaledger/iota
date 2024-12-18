#!/usr/bin/env bash

# This script creates a versioned documentation directory structure from a fixed source directory.
# The source directory is "../content".
# The destination directory is "versioned-docs/version-$CLI_PARAM" where $CLI_PARAM is provided by the user.
#
# Usage:
#   ./recreate_structure_with_symlinks.sh <version_number>
#
# Example:
#   ./recreate_structure_with_symlinks.sh 2
#
# This will:
#   - Set SRC_DIR to "../content"
#   - Set DST_DIR to "versioned-docs/version-2"
#   - Recursively re-create the directory structure from SRC_DIR into DST_DIR
#   - Create symlinks for all files in SRC_DIR into DST_DIR

if [ $# -ne 1 ]; then
    echo "Usage: $0 <version_number>"
    exit 1
fi

CLI_PARAM="$1"
SRC_DIR="../../content"
DST_DIR="../versioned_docs/version-$CLI_PARAM"


# # Create the target directory if it doesn't exist
mkdir -p "$DST_DIR"

# Convert to absolute paths to avoid confusion with relative directories
SRC_DIR="$(readlink -f "$SRC_DIR")"
DST_DIR="$(readlink -f "$DST_DIR")"

# # Verify source directory exists
if [ ! -d "$SRC_DIR" ]; then
    echo "Error: Source directory '$SRC_DIR' does not exist."
    exit 1
fi
# Recreate all directories from SRC_DIR into DST_DIR
echo "Recreating directory structure in $DST_DIR..."
find "$SRC_DIR" -type d | while read -r dir; do
    rel_path="${dir#$SRC_DIR}"
    mkdir -p "$DST_DIR/$rel_path"
done

# Create symlinks for all files
echo "Creating symlinks for files..."
find "$SRC_DIR" -type f | while read -r file; do
    rel_path="${file#$SRC_DIR}"
    ln -s "$file" "$DST_DIR/$rel_path"
done

chmod -R 777 "$DST_DIR"
echo "Done! The versioned docs are located at: $DST_DIR"
