#!/bin/sh

# Create temporary directory to work in
mkdir tmp
cd tmp

# Create the target directory structure if it doesn't exist
mkdir -p ../../content/developer/iota-notarization/single-notarization/references/wasm
mkdir -p ../../content/developer/iota-notarization/audit-trail/references/wasm

# Create additional temporary directories for single-notarization and audit-trail
mkdir single-notarization
mkdir audit-trail

# Download and copy single-notarization docs
cd single-notarization
curl -sL https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/iota-notarization/0.1/wasm.tar.gz | tar xzv
cp -Rv ./docs/wasm/* ../../../content/developer/iota-notarization/single-notarization/references/wasm/

# Download and copy audit-trail docs
cd ../audit-trail
curl -sL https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/iota-audit-trail/0.1/audit-trail-wasm.tar.gz | tar xzv
# For unknown reasons the path to the audit-trail docs in the extracted tar archive has an additional folder `audit-trail-docs`:
# * Path for single-notarization: ./docs/wasm/*
# * Path for audit-trail:         ./audit-trail-docs/docs/wasm/*
cp -Rv ./audit-trail-docs/docs/wasm/* ../../../content/developer/iota-notarization/audit-trail/references/wasm/

# Return to root and cleanup
cd ../..
rm -rf tmp
