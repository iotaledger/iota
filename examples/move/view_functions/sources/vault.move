// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module view_functions::vault {
    /// A shared, generic vault holding a single value of type `T`.
    public struct Vault<T: store> has key {
        id: UID,
        item: T,
    }

    /// Create and share a vault wrapping `item`.
    public fun create<T: store>(item: T, ctx: &mut TxContext) {
        transfer::share_object(Vault { id: object::new(ctx), item });
    }

    /// Returns an immutable reference to the stored item.
    ///
    /// The view itself places no constraint on `T`: a type parameter used only
    /// behind a reference may be unconstrained.
    #[view]
    public fun item<T: store>(vault: &Vault<T>): &T {
        &vault.item
    }

    /// Returns the value passed in, by copy.
    ///
    /// A type parameter passed by value must have `copy` or `drop`.
    #[view]
    public fun echo<T: copy + drop>(value: T): T {
        value
    }

    /// Returns the number of elements in a vector passed by immutable reference.
    ///
    /// `T` is unconstrained here because the vector is taken by reference.
    #[view]
    public fun count<T>(items: &vector<T>): u64 {
        items.length()
    }
}
