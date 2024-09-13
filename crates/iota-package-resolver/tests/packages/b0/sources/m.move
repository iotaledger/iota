// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[allow(unused_field)]
module b::m {
    use a::m::T2 as M;
    use a::n::T0 as N;

    struct T0 {
        m: M,
        n: N,
    }
}
