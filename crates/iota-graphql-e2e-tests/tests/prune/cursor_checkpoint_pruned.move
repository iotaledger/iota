// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Exercises GraphQL pagination with a cursor viewed at a checkpoint that has
// been pruned from the `checkpoints` table, with no historical fallback
// configured. Covers:
// - `Query.transactionBlocks` without a filter and with a lookup table filter
// - `Query.events` without a filter and with a lookup table filter

//# init --protocol-version 12 --addresses Test=0x0 --accounts A B --simulator --epochs-to-keep 1

//# publish
module Test::M {
    use iota::event;

    public struct Foo has key, store { id: UID, v: u64 }
    public struct Sent has copy, drop { recipient: address }

    public entry fun send(recipient: address, ctx: &mut TxContext) {
        event::emit(Sent { recipient });
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

//# create-checkpoint

//# create-checkpoint

//# create-checkpoint

//# run-graphql --wait-for-checkpoint-pruned 6
# A: the most recent transactions and events are still in the database. The
# cursors printed here are viewed at the latest checkpoint.
{
  transactionBlocks(last: 1, filter: { sentAddress: "@{A}" }) {
    pageInfo { startCursor }
    nodes { digest }
  }
  events(last: 1, filter: { sender: "@{A}" }) {
    pageInfo { startCursor }
    nodes { sendingModule { name } }
  }
}

//# run-graphql --cursors {"c":1,"t":1,"i":false}
# B: transactions without a filter, from a cursor viewed at a pruned
# checkpoint — the request errors with DATA_PRUNED.
{
  transactionBlocks(after: "@{cursor_0}") {
    nodes { digest }
  }
}

//# run-graphql --cursors {"c":1,"t":1,"i":false}
# C: transactions with a lookup table filter, from a cursor viewed at a pruned
# checkpoint — the request errors with DATA_PRUNED.
{
  transactionBlocks(filter: { sentAddress: "@{A}" }, after: "@{cursor_0}") {
    nodes { digest }
  }
}

//# run-graphql --cursors {"tx":1,"e":0,"c":1}
# D: events without a filter, from a cursor viewed at a pruned checkpoint —
# the request errors with DATA_PRUNED.
{
  events(after: "@{cursor_0}") {
    nodes { sendingModule { name } }
  }
}

//# run-graphql --cursors {"tx":1,"e":0,"c":1}
# E: events with a lookup table filter, from a cursor viewed at a pruned
# checkpoint — the request errors with DATA_PRUNED.
{
  events(filter: { sender: "@{A}" }, after: "@{cursor_0}") {
    nodes { sendingModule { name } }
  }
}

//# run-graphql --cursors {"c":10,"t":7,"i":false}
# F: transactions with a lookup table filter, from a cursor viewed at a
# checkpoint that is still in the database — pagination resumes.
{
  transactionBlocks(last: 1, filter: { sentAddress: "@{A}" }, before: "@{cursor_0}") {
    nodes { digest }
  }
}

//# run-graphql --cursors {"tx":7,"e":0,"c":10}
# G: events with a lookup table filter, from a cursor viewed at a checkpoint
# that is still in the database — pagination resumes.
{
  events(last: 1, filter: { sender: "@{A}" }, before: "@{cursor_0}") {
    nodes { sendingModule { name } }
  }
}
