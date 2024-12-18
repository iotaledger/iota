# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2024 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
INSTANCE_ID=${1:-iota}
command=(
  cbt
  -instance
  "$INSTANCE_ID"
)
if [[ -n $BIGTABLE_EMULATOR_HOST ]]; then
  command+=(-project emulator)
fi

for table in objects transactions checkpoints checkpoints_by_digest; do
  (
    set -x
    "${command[@]}" createtable $table
    "${command[@]}" createfamily $table iota
    "${command[@]}" setgcpolicy $table iota maxversions=1
  )
done
