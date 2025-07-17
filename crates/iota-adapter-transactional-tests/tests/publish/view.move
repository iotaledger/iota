//# init --addresses Test=0x0
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# publish
module 0x0::M1 {

    // This should fail, but it doesn't run our verifiers.
    #[view]
    public fun no_return(l: u8) {
        let _x = l + 2;
    }
}
