// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Test depending on another unpublished package, which is published
/// along with your own.
module depends::depends_on_basics {
    use examples::object_basics;
    use iota::tx_context::TxContext;

    public entry fun delegate(ctx: &mut TxContext) {
        object_basics::share(ctx);
    }
}
