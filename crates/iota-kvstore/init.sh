# Copyright (c) Mysten Labs, Inc.
# Modifications Copyright (c) 2025 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

INSTANCE_ID=${1:-iota}
PROJECT_ID=$2

command=(
  cbt
  -instance
  "$INSTANCE_ID"
)

if [[ -n $BIGTABLE_EMULATOR_HOST ]]; then
  # Local development: use emulator project
  command+=(-project emulator)
elif [[ -n $PROJECT_ID ]]; then
  # Remote development: use provided project ID
  command+=(-project "$PROJECT_ID")
fi

for table in objects transactions checkpoints checkpoints_by_digest watermark; do
  (
    set -x
    "${command[@]}" createtable $table
    "${command[@]}" createfamily $table iota
    "${command[@]}" setgcpolicy $table iota maxversions=1
  )
done
"${command[@]}" setgcpolicy watermark iota maxage=2d
