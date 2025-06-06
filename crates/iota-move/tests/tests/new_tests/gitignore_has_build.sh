# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# iota-move new example when `example/.gitignore` already contains build/*; it should be unchanged
mkdir example
echo "ignore1" >> example/.gitignore
echo "build/*" >> example/.gitignore
echo "ignore2" >> example/.gitignore
iota-move new example
cat example/.gitignore
echo
echo ==== files in example/ ====
ls -A example