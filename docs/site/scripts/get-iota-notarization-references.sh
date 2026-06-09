#!/bin/sh

# Create temporary directory to work in
mkdir tmp
cd tmp

# Download and copy docs
curl -sL https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/iota-notarization/0.1/wasm.tar.gz  | tar xzv
# Create the target directory structure if it doesn't exist
mkdir -p ../../content/developer/iota-notarization/references/wasm
cp -Rv ./notarization-docs/docs/wasm/* ../../content/developer/iota-notarization/references/wasm/

# Work around a malformed anchor link in the upstream-generated TypeDoc output:
# LockMetadata.md links to "NotarizationClient.mdupdatemetadata", missing the '#'
# before the anchor. Docusaurus (onBrokenLinks: throw) fails the build on it.
# Remove once https://github.com/iotaledger/notarization regenerates the link correctly.
lock_metadata=../../content/developer/iota-notarization/references/wasm/notarization_wasm/classes/LockMetadata.md
tmp_file=$(mktemp)
sed 's|NotarizationClient\.mdupdatemetadata|NotarizationClient.md#updatemetadata|g' "$lock_metadata" > "$tmp_file" && mv "$tmp_file" "$lock_metadata"

# Return to root and cleanup
cd -
rm -rf tmp
