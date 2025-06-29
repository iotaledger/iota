#!/bin/sh

# Create temporary directory to work in
mkdir tmp
cd tmp

# Download and copy iscmagic docs
curl -sL https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/wasp/2.0/iscmagic.tar.gz | tar xzv

cp -Rv docs/iscmagic/* ../../content/developer/iota-evm/references/magic-contract/

# Download and copy iscutils docs
curl -sL https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/wasp/2.0/iscutils.tar.gz | tar xzv

cp -Rv docs/iscutils ../../content/developer/iota-evm/references/

# Return to root and cleanup
cd -
rm -rf tmp
