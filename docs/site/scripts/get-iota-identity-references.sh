#!/bin/sh

# Create temporary directory to work in
mkdir tmp
cd tmp

# Download and copy docs
curl -sL https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/iota-identity/1.6/wasm.tar.gz  | tar xzv
cp -Rv identity_wasm/docs/wasm/* ../../content/references/iota-identity/wasm/

# Return to root and cleanup
cd -
rm -rf tmp
