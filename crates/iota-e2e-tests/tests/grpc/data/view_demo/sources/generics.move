// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Generic `#[view]` functions: a type-parameter-only return, a generic
/// value argument/return, and a generic object argument.
module view_demo::generics {
    use std::ascii::String;
    use std::type_name;

    public struct Box<T> has key {
        id: UID,
        item: T,
    }

    fun init(ctx: &mut TxContext) {
        transfer::share_object(Box<u64> {
            id: object::new(ctx),
            item: 7,
        });
    }

    /// Result depends only on the type argument.
    #[view]
    public fun type_name_of<T>(): String {
        type_name::get<T>().into_string()
    }

    #[view]
    public fun echo<T: copy + drop>(x: T): T {
        x
    }

    #[view]
    public fun boxed_item<T>(b: &Box<T>): &T {
        &b.item
    }
}
