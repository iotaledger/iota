// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module a::test_false_positives {
    use iota::object::UID;
    use iota::transfer;

    struct NoCap has key {
       id: UID
    }

    struct CapAndHat has key {
       id: UID
    }

    struct Recap has key {
       id: UID
    }

    struct MyCapybara has key {
       id: UID
    }

    public fun freeze_capture(w: NoCap) {
        transfer::public_freeze_object(w);
    }

    public fun freeze_handicap(w: CapAndHat) {
        transfer::public_freeze_object(w);
    }

    public fun freeze_recap(w: Recap) {
        transfer::public_freeze_object(w);
    }

    public fun freeze_capybara(w: MyCapybara) {
        transfer::public_freeze_object(w);
    }
}

module iota::object {
    struct UID has store {
        id: address,
    }
}

module iota::transfer {
    const ZERO: u64 = 0;
    public fun public_freeze_object<T: key>(_: T) {
        abort ZERO
    }
}
