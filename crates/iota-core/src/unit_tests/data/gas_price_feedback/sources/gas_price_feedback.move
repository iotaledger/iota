// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module gas_price_feedback::gas_price_feedback {
    public struct Counter has key, store {
        id: UID,
        value: u64,
    }

    public entry fun create_shared_counter(ctx: &mut TxContext) {
        transfer::public_share_object(Counter { id: object::new(ctx), value: 0 })
    }

    public entry fun increment_both(counter_1:  &mut Counter, counter_2:  &mut Counter) {
        let value = counter_1.value + 1;
        counter_1.value = value;

        let value = counter_2.value + 1;
        counter_2.value = value;
    }

    public entry fun increment_first_read_second(counter_1:  &mut Counter, counter_2:  &Counter) {
        let value = counter_1.value + 1;
        counter_1.value = value;

        let value = counter_2.value;
    }

    public entry fun read_first_increment_second(counter_1:  &Counter, counter_2:  &mut Counter) {
        let value = counter_1.value;

        let value = counter_2.value + 1;
        counter_2.value = value;
    }

    public entry fun read_both(counter_1:  &Counter, counter_2:  &Counter) {
        let value = counter_1.value;

        let value = counter_2.value;
    }

    public entry fun increment_one(counter:  &mut Counter) {
        let value = counter.value + 1;
        counter.value = value;
    }

    public entry fun read_one(counter:  &Counter) {
        let value = counter.value;
    }
}
