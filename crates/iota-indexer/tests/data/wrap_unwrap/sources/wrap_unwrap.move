// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Test module for wrapping and unwrapping objects
module wrap_unwrap::wrap_unwrap {

    public struct Inner has key, store {
        id: UID,
    }

    public struct Wrapper has key, store {
        id: UID,
        inner: Inner,
    }

    public entry fun create_and_wrap(ctx: &mut TxContext) {
        let inner = Inner { id: object::new(ctx) };
        iota::transfer::public_transfer(
            Wrapper { id: object::new(ctx), inner },
            tx_context::sender(ctx),
        );
    }

    public entry fun unwrap(wrapper: Wrapper, ctx: &mut TxContext) {
        let Wrapper { id, inner } = wrapper;
        object::delete(id);
        iota::transfer::public_transfer(inner, tx_context::sender(ctx));
    }
}
