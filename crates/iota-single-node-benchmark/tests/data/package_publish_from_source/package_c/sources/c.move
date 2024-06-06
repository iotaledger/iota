// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module c::c {
    use a::a;
    use b::b;

    public fun c() {
        a::a();
        b::b();
    }
}