#!/bin/sh

# Create temporary directory to work in
mkdir -p tmp
cd tmp || exit

# Download and extract the typedoc tarball
curl -sL "https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/ts-sdk-typedoc.tar.gz" | tar xzv

packages="typescript dapp-kit kiosk bcs signers isc-sdk graphql-transport wallet-standard ledgerjs-hw-app-iota"

for package in $packages; do
    # Copy the package's typedoc to the content directory
    mkdir -p "../../content/developer/ts-sdk/${package}/api/"
    cp -Rv "${package}/"* "../../content/developer/ts-sdk/${package}/api/"
done

# Return to root and cleanup
cd - || exit
rm -rf tmp
