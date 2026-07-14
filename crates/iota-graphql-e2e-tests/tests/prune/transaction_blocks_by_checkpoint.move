// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Exercises GraphQL transaction queries filtered by checkpoint under
// pruning, with no historical fallback configured. Covers:
// - `Query.transactionBlocks(filter: { atCheckpoint })`
// - `Checkpoint.transactionBlocks`

//# init --protocol-version 12 --addresses Test=0x0 --accounts A --simulator --epochs-to-keep 1

//# publish
module Test::M {
    public struct Foo has key, store { id: UID, v: u64 }

    public entry fun create(ctx: &mut TxContext) {
        transfer::public_transfer(Foo { id: object::new(ctx), v: 0 }, ctx.sender())
    }
}

//# run Test::M::create --sender A

//# create-checkpoint

//# advance-epoch

//# create-checkpoint

//# advance-epoch

//# create-checkpoint

//# advance-epoch

//# create-checkpoint

//# run Test::M::create --sender A

//# create-checkpoint

//# create-checkpoint

//# create-checkpoint

//# run-graphql --wait-for-checkpoint-pruned 6
# A: Query.transactionBlocks(filter: { atCheckpoint: <pruned> }) — no fallback.
# The checkpoint has been pruned, so the request errors with DATA_PRUNED.
{
  transactionBlocks(filter: { atCheckpoint: 1 }) {
    nodes { digest }
  }
}

//# run-graphql
# B: Query.transactionBlocks(filter: { atCheckpoint: <unpruned> }) resolves.
{
  transactionBlocks(filter: { atCheckpoint: 8 }) {
    nodes { digest }
  }
}

//# run-graphql
# C: Checkpoint.transactionBlocks on a pruned checkpoint — no fallback.
# The parent `checkpoint(...)` resolves to null, so no nested field is
# fetched.
{
  checkpoint(id: { sequenceNumber: 1 }) {
    transactionBlocks {
      nodes { digest }
    }
  }
}

//# run-graphql
# D: Checkpoint.transactionBlocks on an unpruned checkpoint.
{
  checkpoint(id: { sequenceNumber: 8 }) {
    transactionBlocks {
      nodes { digest }
    }
  }
}

//# run Test::M::create --sender A

//# run Test::M::create --sender A

//# run Test::M::create --sender A

//# create-checkpoint

//# run-graphql
# E: a checkpoint holding several transactions. The cursors printed here are
# the inputs for the window in F.
{
  transactionBlocks(filter: { atCheckpoint: 11 }) {
    pageInfo { startCursor endCursor }
    nodes { digest }
  }
}

//# run-graphql --cursors {"c":11,"t":7,"i":false} {"c":11,"t":9,"i":false}
# F: both cursors set — the window between the first and the last transaction
# of checkpoint 11. Only the middle transaction is inside the window
# (cursors are exclusive). Both page flags are true because transactions
# exist at the cursor positions, outside the window.
{
  transactionBlocks(filter: { atCheckpoint: 11 }, after: "@{cursor_0}", before: "@{cursor_1}") {
    pageInfo { hasPreviousPage hasNextPage }
    nodes { digest }
  }
}

//# run-graphql --cursors {"c":11,"t":3,"i":false}
# G: `after` cursor below the checkpoint's transaction range. No transaction
# in this checkpoint sits at the cursor, so the bound is invalid and the
# connection comes back empty, as for other transaction queries.
{
  transactionBlocks(filter: { atCheckpoint: 11 }, after: "@{cursor_0}") {
    pageInfo { hasPreviousPage hasNextPage }
    nodes { digest }
  }
}
