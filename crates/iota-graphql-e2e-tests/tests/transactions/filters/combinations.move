// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Tests which filter combinations `transactionBlocks` accepts. The filters
// that read a lookup table of their own (function, kind, recvAddress,
// affectedAddress, inputObject, changedObject, wrappedOrDeletedObject) cannot
// be combined with each other. Each of them can be combined with sentAddress,
// transactionIds and the checkpoint bounds.

//# init --protocol-version 15 --addresses Test=0x0 --accounts A B --simulator

//# publish
module Test::M1 {
    public struct Object has key, store {
        id: UID,
        value: u64,
    }

    public entry fun create(value: u64, recipient: address, ctx: &mut TxContext) {
        transfer::public_transfer(
            Object { id: object::new(ctx), value },
            recipient
        )
    }

    public entry fun consume(object: Object) {
        let Object { id, value: _ } = object;
        id.delete();
    }
}

//# create-checkpoint

//# run Test::M1::create --args 0 @B --sender A

//# run Test::M1::create --args 1 @B --sender A

//# run Test::M1::consume --args object(3,0) --sender B

//# create-checkpoint

//# run-graphql
# Every pair of lookup table filters is rejected, and so is any larger set.
{
  functionAndRecipient: transactionBlocks(filter: {function: "@{Test}::M1::create" recvAddress: "@{B}"}) {
    nodes { digest }
  }
  functionAndKind: transactionBlocks(filter: {function: "@{Test}::M1::create" kind: PROGRAMMABLE_TX}) {
    nodes { digest }
  }
  recipientAndAffected: transactionBlocks(filter: {recvAddress: "@{B}" affectedAddress: "@{A}"}) {
    nodes { digest }
  }
  inputAndChanged: transactionBlocks(filter: {inputObject: "@{obj_3_0}" changedObject: "@{obj_3_0}"}) {
    nodes { digest }
  }
  changedAndWrappedOrDeleted: transactionBlocks(filter: {changedObject: "@{obj_3_0}" wrappedOrDeletedObject: "@{obj_3_0}"}) {
    nodes { digest }
  }
  threeFilters: transactionBlocks(filter: {function: "@{Test}::M1::create" recvAddress: "@{B}" inputObject: "@{obj_3_0}"}) {
    nodes { digest }
  }
}

//# run-graphql
# Combined with the sender, the checkpoint bounds or the transaction ids, each
# of them is accepted.
{
  functionBySender: transactionBlocks(filter: {function: "@{Test}::M1::create" sentAddress: "@{A}"}) {
    nodes { digest }
  }
  recipientBySenderInRange: transactionBlocks(filter: {recvAddress: "@{B}" sentAddress: "@{A}" afterCheckpoint: 1 beforeCheckpoint: 3}) {
    nodes { digest }
  }
  changedAtCheckpoint: transactionBlocks(filter: {changedObject: "@{obj_3_0}" atCheckpoint: 2}) {
    nodes { digest }
  }
  wrappedOrDeletedBySender: transactionBlocks(filter: {wrappedOrDeletedObject: "@{obj_3_0}" sentAddress: "@{B}"}) {
    nodes { digest }
  }
  kindBySender: transactionBlocks(filter: {kind: PROGRAMMABLE_TX sentAddress: "@{B}"}) {
    nodes { digest }
  }
}
