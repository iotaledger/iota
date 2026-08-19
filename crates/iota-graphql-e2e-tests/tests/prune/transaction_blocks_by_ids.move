// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Exercises GraphQL transactions filtered by transaction IDs under pruning,
// with no historical fallback configured. Covers:
// - `Query.transactionBlocks(filter: { transactionIds })` for pruned and
//   unpruned digests
// - pagination over the selected transactions

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

//# run Test::M::create --sender A

//# run Test::M::create --sender A

//# run Test::M::create --sender A

//# create-checkpoint

//# create-checkpoint

//# create-checkpoint

//# run-graphql --wait-for-checkpoint-pruned 6
# A: a pruned digest and an unpruned digest requested together. Without a
# fallback the pruned transaction is dropped from the result, and only the
# unpruned one is returned.
{
  transactionBlocks(filter: { transactionIds: ["@{digest_2}", "@{digest_9}"] }) {
    nodes { digest }
  }
}

//# run-graphql
# B: the three unpruned transactions, ordered by sequence number.
{
  transactionBlocks(filter: { transactionIds: ["@{digest_9}", "@{digest_10}", "@{digest_11}"] }) {
    pageInfo { startCursor endCursor }
    nodes { digest }
  }
}

//# run-graphql
# C: forward page limited to two transactions.
{
  transactionBlocks(filter: { transactionIds: ["@{digest_9}", "@{digest_10}", "@{digest_11}"] }, first: 2) {
    pageInfo { hasPreviousPage hasNextPage }
    nodes { digest }
  }
}

//# run-graphql
# D: backward page limited to two transactions.
{
  transactionBlocks(filter: { transactionIds: ["@{digest_9}", "@{digest_10}", "@{digest_11}"] }, last: 2) {
    pageInfo { hasPreviousPage hasNextPage }
    nodes { digest }
  }
}
