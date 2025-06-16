// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota_system::storage_fund;

use iota::balance::{Self, Balance};
use iota::iota::IOTA;

/// Struct representing the storage fund, containing two `Balance`s:
/// - `total_object_storage_rebates` has the invariant that it's the sum of `storage_rebate` of
///    all objects currently stored on-chain. To maintain this invariant, the only inflow of this
///    balance is storage charges collected from transactions, and the only outflow is storage rebates
///    of transactions, including both the portion refunded to the transaction senders as well as
///    the non-refundable portion taken out and put into `non_refundable_balance`.
/// - `non_refundable_balance` contains any remaining inflow of the storage fund that should not
///    be taken out of the fund.
public struct StorageFundV1 has store {
    total_object_storage_rebates: Balance<IOTA>,
    non_refundable_balance: Balance<IOTA>,
}

/// Called by `iota_system` at genesis time.
public(package) fun new(initial_fund: Balance<IOTA>): StorageFundV1 {
    StorageFundV1 {
        // At the beginning there's no object in the storage yet
        total_object_storage_rebates: balance::zero(),
        non_refundable_balance: initial_fund,
    }
}

/// Called by `iota_system` at epoch change times to process the inflows and outflows of storage fund.
public(package) fun advance_epoch(
    self: &mut StorageFundV1,
    storage_charges: Balance<IOTA>,
    storage_rebate_amount: u64,
    non_refundable_storage_fee_amount: u64,
): Balance<IOTA> {
    // The storage charges for the epoch come from the storage rebate of the new objects created
    // and the new storage rebates of the objects modified during the epoch so we put the charges
    // into `total_object_storage_rebates`.
    self.total_object_storage_rebates.join(storage_charges);

    // Split out the non-refundable portion of the storage rebate and put it into the non-refundable balance.
    let non_refundable_storage_fee = self
        .total_object_storage_rebates
        .split(non_refundable_storage_fee_amount);
    self.non_refundable_balance.join(non_refundable_storage_fee);

    // `storage_rebates` include the already refunded rebates of deleted objects and old rebates of modified objects and
    // should be taken out of the `total_object_storage_rebates`.
    let storage_rebate = self.total_object_storage_rebates.split(storage_rebate_amount);

    // The storage rebate has already been returned to individual transaction senders' gas coins
    // so we return the balance to be burnt at the very end of epoch change.
    storage_rebate
}

public fun total_object_storage_rebates(self: &StorageFundV1): u64 {
    self.total_object_storage_rebates.value()
}

public fun total_balance(self: &StorageFundV1): u64 {
    self.total_object_storage_rebates.value() + self.non_refundable_balance.value()
}
