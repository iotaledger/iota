// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//# init --addresses Test=0x0

//# publish --upgradeable
module Test::M1 {
    fun init(_ctx: &mut TxContext) { }
}

//# view-object 1,1
