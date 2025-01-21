// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module template::genesis {

    // IOTA imports
    use iota::package::{ Self };

    public struct GENESIS has drop {}

    fun init (otw: GENESIS, ctx: &mut TxContext){
        package::claim_and_keep(otw, ctx);
    }

    #[test_only]
    public fun init_for_testing(ctx: &mut TxContext) {
        init(GENESIS{}, ctx);
    }
}