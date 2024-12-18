// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module z::a {
    public fun bar(x: u64): u64 {
        z::b::foo(iota::math::max(x, 42))
    }
}
