// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Exercises GraphQL transaction queries filtered by affected address under
// pruning, with no historical fallback configured. Covers:
// - `Query.transactionBlocks(filter: { affectedAddress })`
// - `Address.transactionBlocks` with the `AFFECTED` relation

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
# A: paginating forward starts at the earliest transaction that affected the
# address, which has been pruned — no fallback, so the request errors with
# DATA_PRUNED.
{
  transactionBlocks(filter: { affectedAddress: "@{A}" }) {
    nodes { digest }
  }
}

//# run-graphql
# B: paginating backward starts at the most recent transactions, which are
# still in the database. The cursors printed here are the inputs for the
# window in D.
{
  transactionBlocks(last: 3, filter: { affectedAddress: "@{B}" }) {
    pageInfo { startCursor endCursor hasPreviousPage hasNextPage }
    nodes { digest }
  }
}

//# run-graphql
# C: the AFFECTED relation on Address paginates the same way.
{
  address(address: "@{B}") {
    transactionBlocks(last: 2, relation: AFFECTED) {
      pageInfo { hasPreviousPage hasNextPage }
      nodes { digest }
    }
  }
}

//# run-graphql --cursors {"c":10,"t":6,"i":false} {"c":10,"t":8,"i":false}
# D: both cursors set — the window between the first and the last of the
# recent transactions. Only the middle transaction is inside the window
# (cursors are exclusive). Both page flags are true because transactions
# exist at the cursor positions, outside the window.
{
  transactionBlocks(filter: { affectedAddress: "@{B}" }, after: "@{cursor_0}", before: "@{cursor_1}") {
    pageInfo { hasPreviousPage hasNextPage }
    nodes { digest }
  }
}

//# run-graphql --cursors {"c":10,"t":2,"i":false}
# E: paginating backward from a cursor in the pruned range — no fallback, so
# the request errors with DATA_PRUNED.
{
  transactionBlocks(last: 2, filter: { affectedAddress: "@{B}" }, before: "@{cursor_0}") {
    nodes { digest }
  }
}

//# run-graphql --cursors {"c":10,"t":2,"i":false}
# F: paginating forward from a cursor in the pruned range — no fallback, so
# the request errors with DATA_PRUNED.
{
  transactionBlocks(filter: { affectedAddress: "@{B}" }, after: "@{cursor_0}") {
    nodes { digest }
  }
}

//# run-graphql --cursors {"c":1,"t":1,"i":false}
# G: the cursor is viewed at a checkpoint that has been pruned from the
# `checkpoints` table, so the request errors with DATA_PRUNED.
{
  transactionBlocks(filter: { affectedAddress: "@{B}" }, after: "@{cursor_0}") {
    nodes { digest }
  }
}
