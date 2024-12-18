#!/bin/sh

# Create temporary directory to work in
mkdir tmp
cd tmp

# Download and copy docs
curl -sL https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/testnet/ts
cp -Rv ts ../../docs/content/references/ts-sdk/api

curl -sL https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/testnet/graphql/reference
cp -Rv graphql ../../docs/content/references/iota-api/iota-graphql

curl -sL https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/testnet/ts
cp -Rv rust ../../docs/content/references/framework

# Return to root and cleanup
cd -
rm -rf tmp
