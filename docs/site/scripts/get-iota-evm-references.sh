#!/bin/sh

# Create temporaty directory to work in
mkdir tmp
cd tmp

# Download and copy docs
curl -sL https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/wasp/1.3/iscmagic.tar.gz | tar xzv
cp -Rv docs/iscmagic/* ../../content/references/iota-evm/magic-contract/

curl -sL https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/wasp/1.3/iscutils.tar.gz | tar xzv
cp -Rv docs/iscutils ../../content/references/iota-evm/

# Return to root and cleanup
cd -
rm -rf tmp
