#!/bin/bash

# Create temporary directory to work in
mkdir -p tmp
cd tmp || exit

process() {
    local language="$1"
    local version="$2"
    echo "Processing ${language} SDK version ${version}..."
    curl -sL "https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/iota-sdk/${version}/${language}.tar.gz" | tar xzv

    # Copy framework docs
    mkdir -p "../../content/developer/iota-sdk/references/${language}/"
    cp -Rv docs/${language}* "../../content/developer/iota-sdk/references/"

    # Clean up for the next iteration
    rm -rf docs
}

process "python" "3.0"
# Ignore Go until we found a better solution to produce reference docs for it
#process "go" "0.0"
# Ignore Kotlin until we found a solution to produce references without broken links
#process "kotlin" "0.0"

# Return to root and cleanup
cd - || exit
rm -rf tmp
