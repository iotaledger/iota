// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung 
// SPDX-License-Identifier: Apache-2.0

module d::d {
    use b::b::b;
    use c::c::c;

    public fun d(): u64 {
        let var = 123;
        let _ = var + 4;
        b() + c()
    }
}
