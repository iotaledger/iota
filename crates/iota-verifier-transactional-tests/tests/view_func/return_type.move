// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// view function must have a return type

//# publish
module 0x0::m {

    // this should probably fail, but also doesn't run our verifiers
    #[view]
    public fun no_return(input: u8) {
        let _ = input + 3;
    }
}