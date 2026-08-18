// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Emits an event from `init`, so that simulating the publish produces an event
/// whose type only the just-published package defines. Decoding it requires
/// resolving that type against the simulation's own output rather than the
/// database, which cannot hold a package that was never committed.
module publish_with_event::publish_with_event {
    use std::ascii::{Self, String};

    use iota::event;

    public struct PublishEvent has copy, drop {
        foo: String,
    }

    fun init(_ctx: &mut TxContext) {
        event::emit(PublishEvent { foo: ascii::string(b"bar") })
    }
}
