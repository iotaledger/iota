// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Exercises the paginated `transactionsByDigests` query. Covers:
// - nodes keep the order of the `digests` argument
// - a digest that is not found keeps its (null) entry in the page
// - forward (`limit`) pages and the `hasNextPage` flag
// - resuming from a cursor (`cursor`)
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
# A: all five digests, in the order of the input, and the end cursor is set.
{
  transactionsByDigests(digests: ["@{digest_4}", "@{digest_7}", "@{digest_2}", "@{digest_6}", "@{digest_3}"]) {
    hasNextPage endCursor
    nodes { digest }
  }
}

//# run-graphql
# B: forward page limited to the first two entries of the input.
{
  transactionsByDigests(limit: 2, digests: ["@{digest_4}", "@{digest_7}", "@{digest_2}", "@{digest_6}", "@{digest_3}"]) {
    hasNextPage
    nodes { digest }
  }
}

//# run-graphql
# C: a digest that does not exist keeps its (null) entry in the page.
{
  transactionsByDigests(digests: ["@{digest_4}", "11111111111111111111111111111111", "@{digest_2}"]) {
    hasNextPage
    nodes { digest }
  }
}

//# run-graphql
# D: an empty digest list returns no nodes and a null end cursor.
{
  transactionsByDigests(digests: []) {
    hasNextPage endCursor
    nodes { digest }
  }
}

//# run-graphql --cursors {"c":2,"i":1}
# E: resume after the second entry's cursor returns the remaining three, in
# input order.
{
  transactionsByDigests(cursor: "@{cursor_0}", digests: ["@{digest_4}", "@{digest_7}", "@{digest_2}", "@{digest_6}", "@{digest_3}"]) {
    hasNextPage
    nodes { digest }
  }
}

//# run-graphql --cursors {"c":2,"i":1}
# F: a forward page of two after the second entry's cursor.
{
  transactionsByDigests(cursor: "@{cursor_0}", limit: 2, digests: ["@{digest_4}", "@{digest_7}", "@{digest_2}", "@{digest_6}", "@{digest_3}"]) {
    hasNextPage
    nodes { digest }
  }
}

//# run-graphql
# G: a full page of nulls. The two nonexistent digests keep their entries,
# and `hasNextPage` still points at the next page.
{
  transactionsByDigests(limit: 2, digests: ["11111111111111111111111111111111", "11111111111111111111111111111112", "@{digest_2}"]) {
    hasNextPage endCursor
    nodes { digest }
  }
}

//# run-graphql --cursors {"c":2,"i":1}
# H: resuming after the second null entry returns the real transaction.
{
  transactionsByDigests(cursor: "@{cursor_0}", digests: ["11111111111111111111111111111111", "11111111111111111111111111111112", "@{digest_2}"]) {
    hasNextPage
    nodes { digest }
  }
}
