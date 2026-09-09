// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Exercises GraphQL checkpoint queries under pruning, with no historical
// fallback configured. Covers:
// - top-level `Query.checkpoints` after pruning
// - paginating with a cursor that points below the pruning watermark
// - `Query.checkpoint(sequenceNumber)` for both pruned and unpruned seqs
// - nested `Epoch.checkpoints` on a fully-pruned epoch and on the current one

//# init --protocol-version 12 --addresses Test=0x0 --simulator --epochs-to-keep 1

//# publish
module Test::M {
    public entry fun noop() {}
}

//# create-checkpoint

//# advance-epoch

//# create-checkpoint

//# advance-epoch

//# create-checkpoint

//# advance-epoch

//# create-checkpoint

//# create-checkpoint

//# create-checkpoint

//# run-graphql --wait-for-checkpoint-pruned 6
# A: top-level checkpoints, defaults.
# After waiting for pruning, only unpruned checkpoints come back.
# `hasPreviousPage` must be `false` because we clamp `absolute_lo_incl` to the
# pruning watermark.
{
  checkpoints {
    pageInfo { hasPreviousPage hasNextPage }
    nodes { sequenceNumber }
  }
}

//# run-graphql
# B: top-level checkpoints with `first: 2`.
# `hasNextPage` should be `true` (more unpruned data above), `hasPreviousPage`
# should be `false` (we're at the bottom of the reachable range).
{
  checkpoints(first: 2) {
    pageInfo { hasPreviousPage hasNextPage }
    nodes { sequenceNumber }
  }
}

//# run-graphql --cursors {"c":9,"s":2}
# C: paginate forward from a cursor below the pruning watermark.
# `c` is the view-at checkpoint, `s` is the cursor's sequence number. A cursor
# below the watermark means the referenced data has been pruned -- the request
# is rejected with DATA_PRUNED, telling the client to retry from scratch.
{
  checkpoints(first: 2, after: "@{cursor_0}") {
    pageInfo { hasPreviousPage hasNextPage }
    nodes { sequenceNumber }
  }
}

//# run-graphql --cursors {"c":9,"s":42}
# C2: cursor above the upper bound (viewed_at is 9, but cursor seq is 42).
# The cursor contradicts its own `checkpoint_viewed_at`, so it's malformed --
# return BAD_USER_INPUT.
{
  checkpoints(first: 2, after: "@{cursor_0}") {
    pageInfo { hasPreviousPage hasNextPage }
    nodes { sequenceNumber }
  }
}

//# run-graphql --cursors {"c":9,"s":7}
# D: paginate forward from a cursor sitting at the lower bound (s=7, the
# lowest unpruned seq). The boundary is in-range, so the page returns the
# items strictly above it.
{
  checkpoints(first: 2, after: "@{cursor_0}") {
    pageInfo { hasPreviousPage hasNextPage }
    nodes { sequenceNumber }
  }
}

//# run-graphql
# E: Query.checkpoint(seq) for a pruned seq.
# Should resolve to `null` -- the DataLoader can't find the row in either
# Postgres or the (absent) fallback.
{
  pruned: checkpoint(id: { sequenceNumber: 0 }) {
    sequenceNumber
  }
}

//# run-graphql
# F: Query.checkpoint(seq) for an unpruned seq -- resolves to the row.
{
  unpruned: checkpoint(id: { sequenceNumber: 7 }) {
    sequenceNumber
  }
}

//# run-graphql
# G: nested Epoch.checkpoints on a fully-pruned epoch.
# Epoch 0's range is entirely below the watermark; without a fallback we have
# no way to serve any of its checkpoints. `epochId` resolves, the
# `checkpoints` sub-field errors with DATA_PRUNED.
{
  epoch(id: 0) {
    epochId
    checkpoints {
      pageInfo { hasPreviousPage hasNextPage }
      nodes { sequenceNumber }
    }
  }
}

//# run-graphql
# H: nested Epoch.checkpoints on the unpruned epoch -- paginates normally.
{
  epoch(id: 3) {
    epochId
    checkpoints {
      pageInfo { hasPreviousPage hasNextPage }
      nodes { sequenceNumber }
    }
  }
}
