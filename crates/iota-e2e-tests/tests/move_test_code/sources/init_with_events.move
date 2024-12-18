// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module move_test_code::init_with_event {
    public struct Event has drop, copy {}

    fun init(_ctx: &mut TxContext) {
        iota::event::emit(Event {});
    }
}