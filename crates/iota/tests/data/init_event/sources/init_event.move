// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module init_event::init_event {
    use iota::event;

    public struct Initialized has copy, drop {
        value: u64,
    }

    fun init(_ctx: &mut TxContext) {
        event::emit(Initialized { value: 42 });
    }
}
