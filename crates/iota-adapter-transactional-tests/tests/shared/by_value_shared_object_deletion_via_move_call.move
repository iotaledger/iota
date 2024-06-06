// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# init --addresses t1=0x0 t2=0x0 --shared-object-deletion true

//# publish

module t2::o2 {
    use iota::iota::IOTA;
    use iota::coin::{Self, Coin};

    public struct Obj2 has key, store {
        id: UID,
    }

    public fun mint_shared_coin(ctx: &mut TxContext) {
        transfer::public_share_object(coin::zero<IOTA>(ctx))
    }

    public fun create(ctx: &mut TxContext) {
        let o = Obj2 { id: object::new(ctx) };
        transfer::public_share_object(o)
    }

    public fun deleter(o2: Obj2) {
        let Obj2 { id } = o2;
        object::delete(id);
    }

    public fun id<T>(i: T): T { i }

    public fun share_coin(o2: Coin<IOTA>) {
        transfer::public_share_object(o2);
    }
}

//# run t2::o2::create

//# run t2::o2::create

//# view-object 2,0

//# view-object 3,0

// Make MoveVec and then delete
//# programmable --inputs object(2,0) object(3,0)
//> 0: t2::o2::id<t2::o2::Obj2>(Input(1));
//> 1: t2::o2::deleter(Result(0));

//# run t2::o2::mint_shared_coin

//# view-object 7,0

// Reshare the shared object after making the move v
//# programmable --inputs 0 object(7,0) @0x0
//> 0: t2::o2::id<iota::coin::Coin<iota::iota::IOTA>>(Input(1));
//> 1: SplitCoins(Result(0), [Input(0)]);
//> 2: TransferObjects([Result(1)], Input(2));
//> 3: t2::o2::share_coin(Result(0));

// Try to call public_share_object directly -- this should work because the coin has `store`.
//# programmable --inputs 0 object(7,0) @0x0
//> 0: t2::o2::id<iota::coin::Coin<iota::iota::IOTA>>(Input(1));
//> 1: SplitCoins(Result(0), [Input(0)]);
//> 2: TransferObjects([Result(1)], Input(2));
//> 3: iota::transfer::public_share_object<iota::coin::Coin<iota::iota::IOTA>>(Result(0));
