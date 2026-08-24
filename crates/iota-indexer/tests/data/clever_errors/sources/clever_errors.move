// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module clever_errors::clever_errors {
    #[error]
    const ENotFound: vector<u8> = b"Element not found in vector 💥 🚀 🌠";

    #[error(code = 1)]
    const ECodedError: vector<u8> = b"Coded clever error";

    public fun clever_aborter() {
        assert!(false, ENotFound);
    }

    public fun clever_aborter_with_code() {
        assert!(false, ECodedError);
    }
}
