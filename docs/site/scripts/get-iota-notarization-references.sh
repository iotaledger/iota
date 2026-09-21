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

# The three tar.gz archives use different internal directory layouts:
# * Single Notarization: ./notarization-docs/docs/wasm/*
# * Audit Trails: ./audit-trail-docs/docs/wasm/*
# * Proof of Inclusion: ./proof-of-inclusion-docs/docs/*

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
