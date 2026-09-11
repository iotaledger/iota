#!/bin/sh

# Create temporary directory to work in
mkdir tmp
cd tmp

# Create the target directory structure if it doesn't exist
mkdir -p ../../content/developer/iota-notarization/single-notarization/references/wasm
mkdir -p ../../content/developer/iota-notarization/audit-trails/references/wasm
mkdir -p ../../content/developer/iota-notarization/proof-of-inclusion/references/wasm

# Create a temporary directory for each package.
mkdir single-notarization
mkdir audit-trails
mkdir proof-of-inclusion

# We are going to download the tag.gz files for single-notarization and audit-trails.
# In each tag.gz file, different folder structures are used:
# * Path for single-notarization: ./notarization-docs/docs/wasm/*
# * Path for audit-trail:         ./audit-trail-docs/docs/wasm/*

# Download and copy single-notarization docs
cd single-notarization
curl -sL https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/iota-notarization/0.1/wasm.tar.gz | tar xzv
cp -Rv ./notarization-docs/docs/wasm/* ../../../content/developer/iota-notarization/single-notarization/references/wasm/

# Download and copy audit-trails docs
cd ../audit-trails
curl -sL https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/iota-notarization/0.1/audit-trail-wasm.tar.gz | tar xzv
cp -Rv ./audit-trail-docs/docs/wasm/* ../../../content/developer/iota-notarization/audit-trails/references/wasm/

# Download and copy Proof of Inclusion docs
cd ../proof-of-inclusion
curl -sL https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/iota-notarization/0.1/proof-of-inclusion-wasm.tar.gz | tar xzv
cp -Rv ./proof-of-inclusion-docs/docs/wasm/* ../../../content/developer/iota-notarization/proof-of-inclusion/references/wasm/

# Return to root and cleanup
cd ../..
rm -rf tmp
