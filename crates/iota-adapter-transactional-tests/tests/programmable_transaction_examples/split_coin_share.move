// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# init --addresses p=0x0 q=0x0 q_2=0x0 r=0x0 s=0x0 --accounts A

//# publish
module p::m {
    use iota::iota::IOTA;
    use iota::coin;

    public fun sharer<T: key + store>(x: T) {
        transfer::public_share_object(x);
    }

    public fun mint_shared(ctx: &mut TxContext) {
        transfer::public_share_object(coin::zero<IOTA>(ctx))
    }
}

//# programmable --sender A --inputs 10
//> 0: SplitCoins(Gas, [Input(0)]);
//> 1: iota::transfer::public_share_object<iota::coin::Coin<iota::iota::IOTA>>(Result(0));

//# programmable --sender A --inputs 10
//> 0: SplitCoins(Gas, [Input(0)]);
//> 1: p::m::sharer<iota::coin::Coin<iota::iota::IOTA>>(Result(0));

//# run p::m::mint_shared

//# view-object 4,0

// This is OK -- split off from a shared object and transfer the split-off coin
//# programmable --sender A --inputs 0 object(4,0) @A
//> 0: SplitCoins(Input(1), [Input(0)]);
//> 1: TransferObjects([Result(0)], Input(2));

// This is not OK -- split off from a shared object and transfer shared object
//# programmable --sender A --inputs 0 object(4,0) @A
//> 0: SplitCoins(Input(1), [Input(0)]);
//> 1: TransferObjects([Result(0)], Input(2));
//> 2: TransferObjects([Input(1)], Input(2));