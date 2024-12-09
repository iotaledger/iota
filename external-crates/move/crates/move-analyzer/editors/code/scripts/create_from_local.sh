#!/bin/zsh
# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# This script is meant to be executed on MacOS (hence zsh use - to get associative arrays otherwise
# unavailable in the bundled bash version).

set -e

usage() {
    SCRIPT_NAME=$(basename "$1")
    >&2 echo "Usage: $SCRIPT_NAME  -pkg|-pub [-h] BINDIR"
    >&2 echo ""
    >&2 echo "Options:"
    >&2 echo " -pub          Publish extensions for all targets"
    >&2 echo " -pkg          Package extensions for all targets"
    >&2 echo " -h            Print this message"
    >&2 echo " BINDIR        Directory containing pre-built IOTA move-analyzer binaries"
}

clean_tmp_dir() {
  test -d "$TMP_DIR" && rm -fr "$TMP_DIR"
}

if [[ "$@" == "" ]]; then
    usage $0
    exit 1
fi

BIN_DIR=""
for cmd in "$@"
do
    if [[ "$cmd" == "-h" ]]; then
        usage $0
        exit 0
    elif [[ "$cmd" == "-pkg" ]]; then
        OP="package"
        OPTS="-omove-VSCODE_OS.vsix"
    elif [[ "$cmd" == "-pub" ]]; then
        OP="publish"
        OPTS=""
    else
        BIN_DIR=$cmd

        if [[ ! -d "$BIN_DIR" ]]; then
            echo IOTA binary directory $BIN_DIR does not exist
            usage $0
            exit 1
        fi
    fi
done

if [[ $BIN_DIR == "" ]]; then
    # directory storing IOTA binaries have not been defined
    usage $0
    exit 1
fi

# a map from os version identifiers in IOTA's binary distribution to os version identifiers
# representing VSCode's target platforms used for creating platform-specific plugin distributions
declare -A SUPPORTED_OS
SUPPORTED_OS[macos-arm64]=darwin-arm64
SUPPORTED_OS[macos-x86_64]=darwin-x64
SUPPORTED_OS[ubuntu-x86_64]=linux-x64
SUPPORTED_OS[windows-x86_64]=win32-x64

TMP_DIR=$( mktemp -d -t vscode-createXXX )
trap "clean_tmp_dir $TMP_DIR" EXIT

BIN_FILES=($BIN_DIR/*)

if (( ${#BIN_FILES[@]} != 4 )); then
    echo "IOTA binary directory $BIN_DIR should only contain binaries for the four supported platforms"
    exit 1
fi


for IOTA_MOVE_ANALYZER_BIN in "${BIN_FILES[@]}"; do
    echo "Processing" $IOTA_MOVE_ANALYZER_BIN
    # Extract just the file name
    FILE_NAME=${IOTA_MOVE_ANALYZER_BIN##*/}
    # Get the OS target
    OS_TARGET="${FILE_NAME#move-analyzer-}"
    # Remove ".exe" for Windows
    OS_TARGET="${OS_TARGET%.exe}"

    if [[ ! -v SUPPORTED_OS[$OS_TARGET] ]]; then
        echo "Found IOTA binary archive for a platform that is not supported: $IOTA_MOVE_ANALYZER_BIN"
        echo "Supported platforms:"
        for PLATFORM in ${(k)SUPPORTED_OS}; do
            echo "\t$PLATFORM"
        done
        exit 1
    fi

    # copy move-analyzer binary to the appropriate location where it's picked up when bundling the
    # extension
    LANG_SERVER_DIR="language-server"
    # remove existing one to have only the binary for the target OS
    rm -rf $LANG_SERVER_DIR
    mkdir $LANG_SERVER_DIR

    # Copy renamed
    if [[ "$IOTA_MOVE_ANALYZER_BIN" == *.exe ]]; then
        cp $IOTA_MOVE_ANALYZER_BIN $LANG_SERVER_DIR/move-analyzer.exe
    else
        cp $IOTA_MOVE_ANALYZER_BIN $LANG_SERVER_DIR/move-analyzer
        # Make binaries executable
        chmod +x $LANG_SERVER_DIR/move-analyzer
    fi

    VSCODE_OS=${SUPPORTED_OS[$OS_TARGET]}
    vsce "$OP" ${OPTS//VSCODE_OS/$VSCODE_OS} --target "$VSCODE_OS"

    rm -rf $LANG_SERVER_DIR

done


# build a "generic" version of the extension that does not bundle the move-analyzer binary
vsce "$OP" ${OPTS//VSCODE_OS/generic}
