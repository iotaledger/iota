// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module clever_errors::clever_errors {
    #[error]
    const ENotFound: vector<u8> = b"Element not found in vector 💥 🚀 🌠";

    public fun clever_aborter() {
        assert!(false, ENotFound);
    }
}
