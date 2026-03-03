#!/bin/sh


# Define main network 
main_network="mainnet"

networks="testnet devnet"

# Create temporary directory to work in
mkdir -p tmp
cd tmp || exit

# Download and extract the docs for the current network
curl -sL "https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/iota/${main_network}.tar.gz" | tar xzv

# Copy framework docs
mkdir -p "../../content/developer/references/framework/"
cp -Rv docs/generated-docs/framework/* "../../content/developer/references/framework/"

for network in $networks; do
    # Download and extract the docs for the current network
    curl -sL "https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/iota/${network}.tar.gz" | tar xzv

    # Copy framework docs
    mkdir -p "../../content/developer/references/framework/${network}"
    cp -Rv docs/generated-docs/framework/* "../../content/developer/references/framework/${network}"
done

# Return to root and cleanup
cd - || exit
rm -rf tmp
