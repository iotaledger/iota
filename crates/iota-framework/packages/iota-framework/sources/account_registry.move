// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// This is a unique shared object that is used every time someone wants to create a new precomputed id
module iota::account_registry;

use iota::table::{Self, Table};

/// Sender is not @0x0 the system address.
const ENotSystemAddress: u64 = 0;

/// ID already exists
const EIdAlreadyExists: u64 = 1;

/// Singleton shared object that exposes time to Move calls.
public struct AccountRegistry has key {
    id: UID,
    // Ids of generated accounts
    account_ids: Table<ID, ID>,
}

#[allow(unused_function)]
/// Create and share the singleton AccountRegistry -- this function is
/// called exactly once, during genesis.
fun create(ctx: &mut TxContext) {
    assert!(ctx.sender() == @0x0, ENotSystemAddress);

    transfer::share_object(AccountRegistry {
        id: object::account_registry(),
        // Initialised an empty table.
        account_ids: table::new<ID, ID>(ctx),
    })
}

/// Add new account id to the table
public fun add(self: &mut AccountRegistry, account_id: ID) {
    assert!(!self.account_ids.contains(account_id), EIdAlreadyExists);
    self.account_ids.add(account_id, account_id); // Probably we need another collection
}

/// Remove account id from the table
public fun remove(self: &mut AccountRegistry, account_id: ID) {
    self.account_ids.remove(account_id);
}

// The number of account ids
public fun length(self: &mut AccountRegistry): u64 {
    self.account_ids.length()
}

#[test_only]
/// Expose the functionality of `create()` (usually only done during
/// genesis) for tests that want to create a AccountRegistry.
public fun create_for_testing(ctx: &mut TxContext): AccountRegistry {
    AccountRegistry {
        id: object::new(ctx),
        account_ids: table::new<ID, ID>(ctx),
    }
}

#[test_only]
public fun destroy_for_testing(self: AccountRegistry) {
    let AccountRegistry { id, account_ids } = self;
    account_ids.drop();
    id.delete();
}
