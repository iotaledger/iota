// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Exercises GraphQL transaction pagination under pruning, with no historical
// fallback configured. Covers the `oldestAvailableCheckpoint` field of
// `TransactionBlockConnection`, and `DATA_PRUNED` for a checkpoint range or a
// cursor below it, on:
// - `Query.transactionBlocks` with checkpoint filters, cursors and a scan limit
// - `Epoch.transactionBlocks`

//# init --protocol-version 12 --addresses Test=0x0 --accounts A B --simulator --epochs-to-keep 1

//# publish
module Test::M {
    public struct Foo has key, store { id: UID, v: u64 }

    public entry fun send(recipient: address, ctx: &mut TxContext) {
        transfer::public_transfer(Foo { id: object::new(ctx), v: 0 }, recipient)
    }
}

//# run Test::M::send --sender A --args @B

//# create-checkpoint

//# advance-epoch

//# create-checkpoint

//# advance-epoch

//# create-checkpoint

//# advance-epoch

//# create-checkpoint

//# run Test::M::send --sender A --args @B

//# run Test::M::send --sender A --args @B

//# run Test::M::send --sender A --args @B

//# create-checkpoint

//# create-checkpoint

//# create-checkpoint

//# run-graphql --wait-for-checkpoint-pruned 6
# A: without a filter, the page starts at the first transaction still in the
# database, and reports the checkpoint the data is available from.
{
  transactionBlocks(first: 2) {
    oldestAvailableCheckpoint
    pageInfo { hasPreviousPage hasNextPage }
    nodes { digest }
  }
}

//# run-graphql
# B: a checkpoint range starting below the oldest available checkpoint lists
# the transactions above it.
{
  transactionBlocks(filter: { afterCheckpoint: 2 }) {
    oldestAvailableCheckpoint
    pageInfo { hasPreviousPage hasNextPage }
    nodes { digest }
  }
}

//# run-graphql
# C: a checkpoint range entirely below the oldest available checkpoint errors
# with DATA_PRUNED.
{
  transactionBlocks(filter: { beforeCheckpoint: 5 }) {
    nodes { digest }
  }
}

//# run-graphql
# D: the same for a pruned checkpoint combined with another filter, which is
# served by the general query rather than the fallback-aware one.
{
  transactionBlocks(filter: { atCheckpoint: 1, sentAddress: "@{A}" }) {
    nodes { digest }
  }
}

//# run-graphql --cursors {"c":10,"t":2,"i":false}
# E: a cursor pointing at a pruned transaction errors with DATA_PRUNED.
{
  transactionBlocks(after: "@{cursor_0}") {
    nodes { digest }
  }
}

//# run-graphql
# F: a checkpoint after the one viewed at is an empty page, not an error.
{
  transactionBlocks(filter: { atCheckpoint: 999, sentAddress: "@{A}" }) {
    oldestAvailableCheckpoint
    nodes { digest }
  }
}

//# run-graphql
# G: an epoch whose checkpoints have all been pruned errors with DATA_PRUNED,
# one with a pruned start lists the transactions still available.
{
  pruned: epoch(id: 0) {
    transactionBlocks {
      nodes { digest }
    }
  }
  partlyPruned: epoch(id: 3) {
    transactionBlocks {
      oldestAvailableCheckpoint
      nodes { digest }
    }
  }
}

//# run-graphql
# H: a scan-limited page starts scanning at the first transaction still in the
# database.
{
  transactionBlocks(scanLimit: 2, filter: { sentAddress: "@{A}" }) {
    oldestAvailableCheckpoint
    pageInfo { hasPreviousPage hasNextPage startCursor endCursor }
    nodes { digest }
  }
}
