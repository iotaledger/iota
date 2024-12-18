// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota_system::validator_wrapper {
    use iota::versioned::Versioned;

    public struct ValidatorWrapper has store {
        inner: Versioned
    }
}
