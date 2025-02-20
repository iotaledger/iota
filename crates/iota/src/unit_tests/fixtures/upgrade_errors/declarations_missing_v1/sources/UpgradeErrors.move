// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Module: UpgradeErrors

#[allow(unused_field)]
module upgrades::upgrades {
    // struct missing
    public struct StructToBeRemoved {
        b: u64
    }

    public enum EnumToBeRemoved {
        A,
        B
    }

    public fun fun_to_be_removed(): u64 {
        0
    }
}
