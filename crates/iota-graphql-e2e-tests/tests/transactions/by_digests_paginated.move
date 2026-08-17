// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Exercises the paginated `transactionsByDigests` query. Covers:
// - ordering by digest regardless of the input digest order
// - forward (`first`) and backward (`last`) pages and their page-info flags
// - resuming from a cursor (`after` / `before`)
// - an empty digest list

// Only checkpointed transactions are exercised here, the e2e suite does not support testing optimistic transactions.

//# init --protocol-version 12 --addresses Test=0x0 --accounts A --simulator

//# publish
module Test::M {
    public struct Foo has key, store { id: UID, v: u64 }

    public entry fun create(ctx: &mut TxContext) {
        transfer::public_transfer(Foo { id: object::new(ctx), v: 0 }, ctx.sender())
    }
}

//# run Test::M::create --sender A

//# run Test::M::create --sender A

//# run Test::M::create --sender A

//# create-checkpoint

//# run Test::M::create --sender A

//# run Test::M::create --sender A

//# create-checkpoint

//# run-graphql
# A: all five digests, requested out of order. They are returned in digest order (ascending), and both boundary cursors are set.
{
  transactionsByDigests(digests: ["@{digest_4}", "@{digest_7}", "@{digest_2}", "@{digest_6}", "@{digest_3}"]) {
    pageInfo { hasPreviousPage hasNextPage startCursor endCursor }
    nodes { digest }
  }
}

//# run-graphql
# B: forward page limited to the first two, in digest order.
{
  transactionsByDigests(first: 2, digests: ["@{digest_4}", "@{digest_7}", "@{digest_2}", "@{digest_6}", "@{digest_3}"]) {
    pageInfo { hasPreviousPage hasNextPage }
    nodes { digest }
  }
}

//# run-graphql
# C: backward page limited to the last two, in digest order.
{
  transactionsByDigests(last: 2, digests: ["@{digest_4}", "@{digest_7}", "@{digest_2}", "@{digest_6}", "@{digest_3}"]) {
    pageInfo { hasPreviousPage hasNextPage }
    nodes { digest }
  }
}

//# run-graphql
# D: an empty digest list returns no nodes and null boundary cursors.
{
  transactionsByDigests(digests: []) {
    pageInfo { hasPreviousPage hasNextPage startCursor endCursor }
    nodes { digest }
  }
}

//# run-graphql --cursors {"c":2,"d":"@{digest_3}"}
# E: resume after the first digest's cursor returns the remaining four, in
# digest order.
{
  transactionsByDigests(after: "@{cursor_0}", digests: ["@{digest_4}", "@{digest_7}", "@{digest_2}", "@{digest_6}", "@{digest_3}"]) {
    pageInfo { hasPreviousPage hasNextPage }
    nodes { digest }
  }
}

//# run-graphql --cursors {"c":2,"d":"@{digest_3}"}
# F: a forward page of two after the first digest's cursor.
{
  transactionsByDigests(after: "@{cursor_0}", first: 2, digests: ["@{digest_4}", "@{digest_7}", "@{digest_2}", "@{digest_6}", "@{digest_3}"]) {
    pageInfo { hasPreviousPage hasNextPage }
    nodes { digest }
  }
}

//# run-graphql --cursors {"c":2,"d":"@{digest_2}"}
# G: resume before the last digest's cursor returns the first four.
{
  transactionsByDigests(before: "@{cursor_0}", digests: ["@{digest_4}", "@{digest_7}", "@{digest_2}", "@{digest_6}", "@{digest_3}"]) {
    pageInfo { hasPreviousPage hasNextPage }
    nodes { digest }
  }
}
