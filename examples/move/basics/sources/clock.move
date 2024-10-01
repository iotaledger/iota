// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module basics::clock {
    use iota::{clock::Clock, event};

    public struct TimeEvent has copy, drop, store {
        timestamp_ms: u64,
    }

    entry fun access(clock: &Clock) {
        event::emit(TimeEvent { timestamp_ms: clock.timestamp_ms() });
    }
}
