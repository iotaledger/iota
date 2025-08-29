// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# init --protocol-version 4 --addresses P=0x0 --accounts A B C --simulator

//# publish
module P::M {
    public struct Object has key, store { id: UID, xs: u64 }
    public struct Wrapper has key, store { id: UID, obj: Object }

    public fun new(xs: u64, ctx: &mut TxContext): Object {
        Object { id: object::new(ctx), xs }
    }

    public fun wrap(o: Object, ctx: &mut TxContext): Wrapper {
        Wrapper { id: object::new(ctx), obj: o }
    }

    public fun unwrap(w: Wrapper): Object {
        let Wrapper { id, obj } = w;
        id.delete();
        obj
    }

    public fun destroy(o: Object) {
        let Object { id, xs: _ } = o;
        id.delete();
    }
}

//# programmable --sender A --inputs 1 @A
//> P::M::new(Input(0));
//> P::M::wrap(Result(0));
//> TransferObjects([Result(1)], Input(1))

//# programmable --sender A --inputs object(2,0) @A
//> P::M::unwrap(Input(0));
//> TransferObjects([Result(0)], Input(1))

//# programmable --sender A --inputs object(3,0)
//> P::M::destroy(Input(0))

//# create-checkpoint

//# run-graphql
# Query the transactions “wrapped or deleted” for the wrapped object in task 1 which lives later as obj_3_0.
# Two transactions are expected: one for the `wrap` and one for `delete` operation after the unwrapping.
{
    transactionBlocks(last: 10, filter: { wrappedOrDeletedObject: "@{obj_3_0}" }) { nodes { digest } }
}

//# programmable --sender B --inputs 1 @B
//> P::M::new(Input(0));
//> TransferObjects([Result(0)], Input(1))

//# programmable --sender B --inputs object(7,0) @B
//> P::M::wrap(Input(0));
//> TransferObjects([Result(0)], Input(1))

//# programmable --sender B --inputs object(8,0)
//> P::M::unwrap(Input(0));
//> P::M::destroy(Result(0))

//# create-checkpoint

//# run-graphql
# Query the transactions that either wrapped or deleted the object created in task 1.
# Two transactions are expected: one for the `wrap` and one for the "unwrap then delete" operation.
{
  events: transactionBlocks(
    last: 10,
    filter: { wrappedOrDeletedObject: "@{obj_7_0}" }
  ) { nodes { digest } }
}

//# programmable --sender C --inputs 1 @C
//> P::M::new(Input(0));
//> TransferObjects([Result(0)], Input(1))

//# programmable --sender C --inputs object(12,0) @C
//> P::M::wrap(Input(0));
//> TransferObjects([Result(0)], Input(1))

//# programmable --sender C --inputs object(13,0) @C
//> P::M::unwrap(Input(0));
//> TransferObjects([Result(0)], Input(1))

//# programmable --sender C --inputs object(12,0)
//> P::M::destroy(Input(0))

//# create-checkpoint

//# run-graphql
# Query the transaction that either wrapped or deleted the object created in task 1.
# Two transactions are expected: one for the `wrap` and one for the "unwrap then delete" operation.
{
  events: transactionBlocks(
    last: 10,
    filter: { wrappedOrDeletedObject: "@{obj_12_0}" }
  ) { nodes { digest } }
}
