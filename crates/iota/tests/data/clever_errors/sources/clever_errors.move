// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Do _not_ edit this file (yes, even whitespace!). Editing this file will
// cause the tests that use this module to fail.
module clever_errors::clever_errors {
    #[error]
    const ENotFound: vector<u8> = b"Element not found in vector 💥 🚀 🌠";

    #[error]
    const ENotAString: vector<u64> = vector[1,2,3,4];

    #[error(code = 1)]
    const ECodedError: vector<u8> = b"Coded clever error";

    public fun aborter() {
        abort 0
    }

    public fun aborter_line_no() {
        assert!(false);
    }

    public fun clever_aborter() {
        assert!(false, ENotFound);
    }

    public fun clever_aborter_not_a_string() {
        assert!(false, ENotAString);
    }

    public fun clever_aborter_with_code() {
        assert!(false, ECodedError);
    }
}
