#!/bin/sh
# Copyright (c) The Move Contributors
# Modifications Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

# Modifications Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

TYPE="$(echo "$1" | sed s/^--resolve-move-//)"
PACKAGE="$2"

cat <<EOF
Broken response (not a lock file) from resolver for $TYPE of $PACKAGE.
EOF
