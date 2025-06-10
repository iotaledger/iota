#!/bin/sh

# Define main network 
main_network="mainnet"

# Define the other networks to process
networks="testnet devnet"

# Create temporary directory to work in
mkdir -p tmp
cd tmp || exit

# Download and extract the docs for the current network
curl -sL "https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/iota/${main_network}.tar.gz" | tar xzv

# Copy framework docs
mkdir -p "../../content/references/framework/"
cp -Rv docs/generated-docs/framework/* "../../content/references/framework/"

# Fix Sidebar for new route
sed -i -e "s/generated-docs\/ts-sdk/ts-sdk\/api/g" docs/generated-docs/ts-sdk/typedoc-sidebar.cjs

# Copy TS SDK docs
mkdir -p "../../content/ts-sdk/api/"
cp -Rv docs/generated-docs/ts-sdk/* "../../content/ts-sdk/api/"

# Clean up for the next iteration
rm -rf generated-docs

for network in $networks; do
    # Download and extract the docs for the current network
    curl -sL "https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/iota/${network}.tar.gz" | tar xzv

    # Copy framework docs
    mkdir -p "../../content/references/framework/${network}/"
    cp -Rv docs/generated-docs/framework/* "../../content/references/framework/${network}/"

    # Fix Sidebar for new route
    sed -i -e "s/generated-docs\/ts-sdk/ts-sdk\/api\/${network}/g" docs/generated-docs/ts-sdk/typedoc-sidebar.cjs

    # Copy TS SDK docs
    mkdir -p "../../content/ts-sdk/api/${network}/"
    cp -Rv docs/generated-docs/ts-sdk/* "../../content/ts-sdk/api/${network}/"

    # Clean up for the next iteration
    rm -rf generated-docs
done

# Return to root and cleanup
cd - || exit
rm -rf tmp
