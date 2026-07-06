#!/bin/bash
set -euo pipefail

# Downloads the IOTA SDK API reference markdown for every binding language.
#
# The tarballs are generated from the iota-rust-sdk revision pinned in
# docs/site/iota-sdk-references.rev and uploaded to S3 by the
# "Upload IOTA SDK References to S3" workflow in this repository. Each
# tarball contains docs/<language>/** which is copied to
# content/developer/iota-sdk/references/.
REV="$(cat "$(dirname "$0")/../iota-sdk-references.rev")"
BASE_URL="https://s3.eu-central-1.amazonaws.com/files.iota.org/iota-wiki/iota-sdk/${REV}"

# Create temporary directory to work in
mkdir -p tmp
cd tmp || exit

process() {
    local language="$1"
    echo "Processing ${language} SDK reference (${REV})..."
    curl -sfL "${BASE_URL}/${language}.tar.gz" | tar xz

    mkdir -p "../../content/developer/iota-sdk/references/"
    cp -R "docs/${language}" "../../content/developer/iota-sdk/references/"

    # Clean up for the next iteration
    rm -rf docs
}

process "python"
process "go"
process "kotlin"
process "csharp"
process "swift"

# Return to root and cleanup
cd - || exit
rm -rf tmp
