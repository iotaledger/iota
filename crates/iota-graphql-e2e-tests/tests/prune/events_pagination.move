// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Exercises GraphQL event pagination under pruning, with no historical
// fallback configured. Covers the `oldestAvailableCheckpoint` field of
// `EventConnection`, and `DATA_PRUNED` for a cursor below it, on:
// - `Query.events` without a filter, with a sender filter and with a
//   transaction digest filter
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

//# run Test::M::bump --sender A --args object(2,0) 2

//# create-checkpoint

//# advance-epoch

//# create-checkpoint

//# advance-epoch

//# create-checkpoint

//# advance-epoch

//# create-checkpoint

//# run Test::M::bump --sender A --args object(2,0) 2

//# create-checkpoint

//# create-checkpoint

//# create-checkpoint

//# run-graphql --wait-for-checkpoint-pruned 6
# A: without a filter, the page lists the events still in the database and
# reports the checkpoint the data is available from.
{
  events {
    oldestAvailableCheckpoint
    pageInfo { hasPreviousPage hasNextPage }
    nodes { json }
  }
}

//# run-graphql
# B: the same with a sender filter, read through a lookup table.
{
  events(filter: { sender: "@{A}" }) {
    oldestAvailableCheckpoint
    nodes { json }
  }
}

//# run-graphql
# C: a transaction digest filter supports the historical fallback. Without
# one configured, the data is available from the same checkpoint.
{
  events(filter: { transactionDigest: "@{digest_11}" }) {
    oldestAvailableCheckpoint
    nodes { json }
  }
}

//# run-graphql --cursors {"tx":3,"e":0,"c":10}
# D: an `after` cursor pointing at a pruned transaction errors with
# DATA_PRUNED.
{
  events(after: "@{cursor_0}") {
    nodes { json }
  }
}

//# run-graphql --cursors {"tx":3,"e":0,"c":10}
# E: the same for a `before` cursor.
{
  events(before: "@{cursor_0}") {
    nodes { json }
  }
}

//# run-graphql
# F: the events of one transaction block come with the transaction and do
# not report a bound.
{
  transactionBlock(digest: "@{digest_11}") {
    effects {
      events {
        oldestAvailableCheckpoint
        nodes { json }
      }
    }
  }
}
