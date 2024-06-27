// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module iota_system::storage_deposits {
    use iota::balance::{Self, Balance};
    use iota::iota::IOTA;

    /// Struct representing the storage deposits fund, containing a `Balance`:
    /// - `storage_balance` tracks the total balance of storage fees collected from transactions.
    public struct StorageDeposits has store {
        refundable_balance: Balance<IOTA>,
        non_refundable_balance: Balance<IOTA>
    }

    /// Called by `iota_system` at genesis time.
    public(package) fun new(initial_balance: Balance<IOTA>) : StorageDeposits {
        StorageDeposits {
            // Initialize the storage deposits balance
            refundable_balance: initial_balance,
            non_refundable_balance: balance::zero()
        }
    }

     /// Called by `iota_system` at epoch change times to process the inflows and outflows of storage deposits.
    public(package) fun advance_epoch(
        self: &mut StorageDeposits,
        storage_charges: Balance<IOTA>,
        storage_rebate_amount: u64,
    ) : Balance<IOTA> {
        self.refundable_balance.join(storage_charges);

        let storage_rebate = self.refundable_balance.split(storage_rebate_amount);

        //TODO: possibly mint and burn tokens here
        // mint_iota(treasury_cap, storage_charges.value(), ctx);
        // burn_iota(treasury_cap, storage_rebate_amount, ctx);
        storage_rebate
    }

    public fun total_balance(self: &StorageDeposits): u64 {
        self.refundable_balance.value() + self.non_refundable_balance.value()
    }
}
