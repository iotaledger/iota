#!/bin/sh

# Create temporary directory to work in
mkdir tmp
cd tmp

# Download and copy docs
curl -sL https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/iota/testnet.tar.gz | tar xzv
cp -Rv generated-docs/ts/* ../../../content/ts-sdk/api
cp -Rv generated-docs/graphql/* ../../../content/references/iota-api/iota-graphql
cp -Rv generated-docs/framework/* ../../../content/references/framework

# Return to root and cleanup
cd -
rm -rf tmp
