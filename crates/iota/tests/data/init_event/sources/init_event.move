// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module init_event::init_event {
    use std::string::String;

    public struct Published has copy, drop {
        note: String,
    }

    fun init(_ctx: &mut TxContext) {
        iota::event::emit(Published { note: std::string::utf8(b"published") });
    }
}
