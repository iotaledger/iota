#!/bin/sh

# Create temporary directory to work in
mkdir tmp
cd tmp

# Download and copy Testnet docs
curl -sL https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/iota/devnet.tar.gz | tar xzv

mkdir  ../../content/references/framework/Testnet/
cp -Rv generated-docs/framework/* ../../content/references/framework/Testnet/

mkdir ../../content/ts-sdk/api/Testnet/
cp -Rv generated-docs/ts/* ../../content/ts-sdk/api/Testnet/

# Download and copy Devnet docs
rm -rf generated-docs
curl -sL https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/iota/devnet.tar.gz | tar xzv

mkdir  ../../content/references/framework/Devnet/
cp -Rv generated-docs/framework/* ../../content/references/framework/Devnet/

mkdir  ../../content/ts-sdk/api/Devnet/
cp -Rv generated-docs/ts/* ../../content/ts-sdk/api/Devnet/


# Return to root and cleanup
cd -
rm -rf tmp
