// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[allow(unused_field)]
module a::n {
    struct T0 {
        t: a::m::T1<u16, u32>,
        u: a::m::T2,
    }
}
