// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota_system::validator_wrapper {
    use iota::versioned::Versioned;

    friend iota_system::iota_system_state_inner;

    struct ValidatorWrapper has store {
        inner: Versioned
    }
}
