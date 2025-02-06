# Copyright (c) Mysten Labs, Inc.
# SPDX-License-Identifier: Apache-2.0

# check that iota move new correctly updates existing .gitignore
mkdir example
echo "existing_ignore" >> example/.gitignore
iota move new example
cat example/.gitignore
