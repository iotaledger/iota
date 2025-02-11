#!/bin/sh

# Create temporary directory to work in
mkdir tmp
cd tmp

# Download and copy Testnet docs
curl -sL https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/iota/testnet.tar.gz | tar xzv

mkdir  ../../content/references/framework/testnet/
cp -Rv generated-docs/framework/* ../../content/references/framework/testnet/

mkdir ../../content/ts-sdk/api/testnet/
cp -Rv generated-docs/ts/* ../../content/ts-sdk/api/testnet/

# Download and copy Devnet docs
rm -rf generated-docs
curl -sL https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/iota/devnet.tar.gz | tar xzv

mkdir  ../../content/references/framework/devnet/
cp -Rv generated-docs/framework/* ../../content/references/framework/devnet/

mkdir  ../../content/ts-sdk/api/devnet/
cp -Rv generated-docs/ts/* ../../content/ts-sdk/api/devnet/


# Return to root and cleanup
cd -
rm -rf tmp
