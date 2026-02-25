#!/bin/sh

# Define the packages to process
packages="typescript graphql-transport kiosk ledgerjs-hw-app-iota wallet-standard dapp-kit"

networks="testnet devnet"

# Copy framework docs
mkdir -p "./docs/content/developer/references/framework/"
cp -Rv ./docs/generated-docs/framework/* "./docs/content/developer/references/framework/"

for network in $networks; do
    # Download and extract the docs for the current network
    curl -sL "https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/iota/${network}.tar.gz" | tar xzv

    # Copy framework docs
    mkdir -p "./docs/content/developer/references/framework/${network}"
    cp -Rv ./docs/generated-docs/framework/* "./docs/content/developer/references/framework/${network}"
done

for package in $packages; do
    # Fix Sidebar for new route
    sed -i -e "s|../generated-docs/ts-sdk/${package}|developer/ts-sdk/${package}/api|g" ./docs/generated-docs/ts-sdk/${package}/typedoc-sidebar.cjs

    # Copy package docs
    mkdir -p "./docs/content/developer/ts-sdk/${package}/api/"
    cp -Rv ./docs/generated-docs/ts-sdk/${package}/* "./docs/content/developer/ts-sdk/${package}/api/"
done

