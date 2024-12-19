#!/bin/sh

# Create temporary directory to work in
mkdir tmp
cd tmp

# Download and copy docs
curl -sL https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/testnet.tar.gz | tar xzv
cp -Rv ts ../../docs/content/references/ts-sdk/api
cp -Rv graphql ../../docs/content/references/iota-api/iota-graphql
cp -Rv rust ../../docs/content/references/framework

# Return to root and cleanup
cd -
rm -rf tmp
