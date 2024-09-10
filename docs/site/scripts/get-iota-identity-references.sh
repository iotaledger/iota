#!/bin/sh

# Create temporaty directory to work in
mkdir tmp
cd tmp

# Download and copy docs
curl -sL https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/iota-identity/1.3/wasm.tar.gz | tar xzv
cp -Rv docs/* ../../content/references/iota-identity/

# Return to root and cleanup
cd -
rm -rf tmp
