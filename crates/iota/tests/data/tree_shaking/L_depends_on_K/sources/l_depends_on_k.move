// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module l_depends_on_k::l_depends_on_k {
    public fun l() {
        let x = 1;
        k::k::k();
    }
}
