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

# A pin bump triggers the upload workflow on the same push this build runs
# for, so the tarballs for a freshly pinned revision may still be uploading;
# wait for them (bounded) instead of failing the build.
all_present() {
    for language in python go kotlin csharp swift; do
        curl -sfIL -o /dev/null "${BASE_URL}/${language}.tar.gz" || return 1
    done
}

for i in $(seq 1 60); do
    if all_present; then
        break
    fi
    if [ "$i" = 60 ]; then
        echo "References for revision ${REV} not found on S3 after 30 minutes." >&2
        echo "Check the 'Upload IOTA SDK References to S3' workflow run." >&2
        exit 1
    fi
    echo "Waiting for references of revision ${REV} to appear on S3..."
    sleep 30
done

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

for language in python go kotlin csharp swift; do
    process "$language"
done

# Return to root and cleanup
cd - || exit
rm -rf tmp
