// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Exercises GraphQL event queries filtered by transaction digest under
// pruning, with no historical fallback configured. Covers:
// - `Query.events(filter: { transactionDigest })`
// - `TransactionBlockEffects.events`

//# init --protocol-version 12 --addresses Test=0x0 --accounts A --simulator --epochs-to-keep 1

//# publish
module Test::M {
    use iota::event;

    public struct Foo has key, store { id: UID, v: u64 }
    public struct Bumped has copy, drop { v: u64 }

    public entry fun create(ctx: &mut TxContext) {
        transfer::public_transfer(Foo { id: object::new(ctx), v: 0 }, ctx.sender())
    }

    public entry fun bump(foo: &mut Foo, times: u64) {
        let mut i = 0;
        while (i < times) {
            foo.v = foo.v + 1;
            event::emit(Bumped { v: foo.v });
            i = i + 1;
        }
    }
}

//# run Test::M::create --sender A

//# run Test::M::bump --sender A --args object(2,0) 3

//# create-checkpoint

//# advance-epoch

//# create-checkpoint

//# advance-epoch

//# create-checkpoint

//# advance-epoch

//# create-checkpoint

//# run Test::M::bump --sender A --args object(2,0) 3

//# create-checkpoint

//# create-checkpoint

//# create-checkpoint

//# run-graphql --wait-for-checkpoint-pruned 6
# A: Query.events on a pruned transaction — no fallback.
# The transaction has been pruned, so the request errors with DATA_PRUNED.
{
  events(filter: { transactionDigest: "@{digest_3}" }) {
    nodes { json }
  }
}

//# run-graphql
# B: Query.events on an unpruned transaction resolves. The cursors printed
# here are the inputs for the window in D.
{
  events(filter: { transactionDigest: "@{digest_11}" }) {
    pageInfo { hasPreviousPage hasNextPage startCursor endCursor }
    nodes { json }
  }
}

//# run-graphql
# C: TransactionBlockEffects.events on an unpruned transaction.
{
  transactionBlock(digest: "@{digest_11}") {
    effects {
      events {
        nodes { json }
      }
    }
  }
}

//# run-graphql --cursors {"tx":7,"e":0,"c":10} {"tx":7,"e":2,"c":10}
# D: both cursors set — the window between the first and the last event of
# the transaction. Only the middle event is inside the window (cursors are
# exclusive). Both page flags are true because events exist at the cursor
# positions, outside the window.
{
  events(filter: { transactionDigest: "@{digest_11}" }, after: "@{cursor_0}", before: "@{cursor_1}") {
    pageInfo { hasPreviousPage hasNextPage }
    nodes { json }
  }
}

//# run-graphql --cursors {"tx":5,"e":0,"c":10}
# E: `after` cursor from a different transaction. No event of this
# transaction sits at the cursor, so the bound is invalid and the connection
# comes back empty, as for other event queries.
{
  events(filter: { transactionDigest: "@{digest_11}" }, after: "@{cursor_0}") {
    pageInfo { hasPreviousPage hasNextPage }
    nodes { json }
  }
}

//# run-graphql
# F: digest combined with another filter — served by the lookup-table path,
# without fallback support.
{
  events(filter: { transactionDigest: "@{digest_11}", sender: "@{A}" }) {
    pageInfo { hasPreviousPage hasNextPage }
    nodes { json }
  }
}
