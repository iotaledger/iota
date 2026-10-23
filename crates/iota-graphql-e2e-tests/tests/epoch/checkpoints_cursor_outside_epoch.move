// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// `Epoch.checkpoints` with a cursor outside the epoch, with nothing pruned.
// Neither case is a pruning error.

//# init --protocol-version 12 --simulator

//# create-checkpoint

//# advance-epoch

//# create-checkpoint

//# advance-epoch

//# create-checkpoint

//# create-checkpoint

//# run-graphql --cursors {"c":6,"s":3}
# A: cursor from epoch 1 (seq 3) used on epoch 2 (seqs 5..=6). The cursor
# is below the epoch's range but not pruned: BAD_USER_INPUT, like a cursor
# above the range.
{
  epoch(id: 2) {
    checkpoints(first: 2, after: "@{cursor_0}") {
      pageInfo { hasPreviousPage hasNextPage }
      nodes { sequenceNumber }
    }
  }
}

//# run-graphql --cursors {"c":4,"s":4}
# B: epoch 2 viewed at checkpoint 4, before the epoch started. There are no
# checkpoints to page over yet: an empty connection, not a DATA_PRUNED error.
{
  epoch(id: 2) {
    checkpoints(first: 2, after: "@{cursor_0}") {
      pageInfo { hasPreviousPage hasNextPage }
      nodes { sequenceNumber }
    }
  }
}
