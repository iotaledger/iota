// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// A module without any function attributes: it gets no on-chain module
/// metadata, so view calls to it fall back to signature checks.
module view_functions::plain {
    public fun forty(): u64 {
        private_forty()
    }

    fun private_forty(): u64 {
        40
    }
}
