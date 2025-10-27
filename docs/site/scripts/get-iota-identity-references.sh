#!/bin/sh

# Create temporary directory to work in
mkdir tmp
cd tmp

# Download and copy docs
curl -sL https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/iota-identity/1.6/wasm.tar.gz  | tar xzv
# Create the target directory structure if it doesn't exist
mkdir -p ../../content/developer/iota-identity/references/wasm
cp -Rv ./docs/wasm/* ../../content/developer/iota-identity/references/wasm/

# Return to root and cleanup
cd -
rm -rf tmp
