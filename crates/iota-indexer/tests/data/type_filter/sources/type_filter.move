// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Types whose names only differ in ways that unescaped `LIKE` cannot tell apart, used to
/// check that object type filters match the requested type and nothing else.
module type_filter::type_filter {
    use iota::object;
    use iota::transfer;
    use iota::tx_context::{Self, TxContext};

    /// The type the tests query for. `_` is the `LIKE` wildcard for "any
    /// single character".
    struct My_Type has key, store {
        id: object::UID,
    }

    /// Differs from `My_Type` only where the `_` is.
    struct MyXType has key, store {
        id: object::UID,
    }

    /// Starts with `My_Type`.
    struct My_TypeExtra has key, store {
        id: object::UID,
    }

    fun init(ctx: &mut TxContext) {
        let sender = tx_context::sender(ctx);

        transfer::public_transfer(My_Type { id: object::new(ctx) }, sender);
        transfer::public_transfer(MyXType { id: object::new(ctx) }, sender);
        transfer::public_transfer(My_TypeExtra { id: object::new(ctx) }, sender);
    }
}
