#!/bin/bash
# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
#
# assumes iota cli installed (brew install iota or cargo build --bin iota)

cd genesis
python3 -m venv .venv
source .venv/bin/activate
python3 -m pip install -r requirements.txt

DIR="files"

if [ -d "$DIR" ]; then
    echo "Directory $DIR exists. Removing..."
    rm -r "$DIR"
fi

echo "Creating directory $DIR..."
mkdir "$DIR"
echo "$DIR directory created."

./generate.py --genesis-template compose-validators.yaml --target-directory "$DIR"
