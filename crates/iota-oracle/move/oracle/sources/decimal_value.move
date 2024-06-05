// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung 
// SPDX-License-Identifier: Apache-2.0

module oracle::decimal_value {
    struct DecimalValue has store, drop, copy {
        value: u64,
        decimal: u8,
    }

    public fun new(value: u64, decimal: u8): DecimalValue {
        DecimalValue { value, decimal }
    }

    public fun value(self: &DecimalValue): u64 {
        self.value
    }

    public fun decimal(self: &DecimalValue): u8 {
        self.decimal
    }
}
