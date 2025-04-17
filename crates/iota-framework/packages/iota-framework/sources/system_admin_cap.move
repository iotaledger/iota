// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// A system admin capability implementation.
module iota::system_admin_cap;

/// The `new` function was called at a non-genesis epoch.
const ENotCalledAtGenesis: u64 = 0;
/// Sender is not @0x0 the system address.
const ENotSystemAddress: u64 = 1;

/// `IotaSystemAdminCap` allows to perform privileged IOTA system operations.
/// For example, packing and unpacking `TimeLock`s during staking, etc.
public struct IotaSystemAdminCap has store {}

#[allow(unused_function)]
/// Create a `IotaSystemAdminCap`.
/// This should be called only once during genesis creation.
fun new_system_admin_cap(ctx: &TxContext): IotaSystemAdminCap {
    assert!(ctx.sender() == @0x0, ENotSystemAddress);
    assert!(ctx.epoch() == 0, ENotCalledAtGenesis);

    IotaSystemAdminCap {}
}

#[test_only]
/// Create a `IotaSystemAdminCap` for testing purposes.
public fun new_system_admin_cap_for_testing(): IotaSystemAdminCap {
    IotaSystemAdminCap {}
}
