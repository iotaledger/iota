// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module a::a {
    use b::b::b;

    public fun a(): u64 {
        b()
    }
}
