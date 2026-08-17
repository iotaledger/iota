// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Exercises GraphQL transaction-by-digest queries under pruning, with no
// historical fallback configured. Covers:
// - `Query.transactionBlock(digest)` for pruned and unpruned digests
// - `Query.transactionBlocksByDigests` for pruned and unpruned digests
// - `Event.transactionBlock` on an event from an unpruned tx
// - `MoveObject.previousTransactionBlock` where the prior-tx digest is pruned

//# init --protocol-version 12 --addresses Test=0x0 --accounts A --simulator --epochs-to-keep 1

//# publish
module Test::M {
    use iota::event;

    public struct Foo has key, store { id: UID, v: u64 }
    public struct Bumped has copy, drop { v: u64 }

    public entry fun create(ctx: &mut TxContext) {
        transfer::public_transfer(Foo { id: object::new(ctx), v: 0 }, ctx.sender())
    }

    public entry fun bump(foo: &mut Foo) {
        foo.v = foo.v + 1;
        event::emit(Bumped { v: foo.v })
    }
}

//# run Test::M::create --sender A

//# create-checkpoint

//# advance-epoch

//# run Test::M::create --sender A

//# create-checkpoint

//# advance-epoch

//# create-checkpoint

//# advance-epoch

//# run Test::M::bump --sender A --args object(2,0)

//# create-checkpoint

//# create-checkpoint

//# create-checkpoint

//# run-graphql --wait-for-checkpoint-pruned 6
# A: pruned digest resolves to null (no fallback).
{
  pruned: transactionBlock(digest: "@{digest_2}") { digest }
}

//# run-graphql
# B: unpruned digest resolves correctly
{
  unpruned: transactionBlock(digest: "@{digest_10}") { digest }
}

//# run-graphql
# C: mixed case - one pruned and one unpruned
{
  transactionBlocksByDigests(digests: ["@{digest_2}", "@{digest_10}"]) {
    digest
  }
}

//# run-graphql
# D: Event.transactionBlock on an event from an unpruned tx
{
  events(filter: { sender: "@{A}" }, last: 1) {
    nodes {
      transactionBlock { digest }
    }
  }
}

//# run-graphql
# E: MoveObject.previousTransactionBlock where the tx is pruned.
{
  object(address: "@{obj_5_0}") {
    asMoveObject {
      previousTransactionBlock { digest }
    }
  }
}
