// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[allow(unused_const)]

module iota::account;

use iota::coin::Coin;
use iota::iota::IOTA;

/// Cannot load an account asset.
/// The address does not have the requested asset.
const EAssetDoesNotExist: u64 = 1;
/// The address has an asset, but the value type does not match.
const EAssetTypeMismatch: u64 = 2;
/// Serialization issue.
const EBCSSerializationFailure: u64 = 3;

/// An account abstraction shared object placeholder.
/// It is not a part of this changes and used just as an example.
public struct Account has key {
    id: UID,
    owner: address,
}

/// Creates a new `Account` shared instance.
public fun create(ctx: &mut TxContext) {
    let id = object::new(ctx);
    let owner = ctx.sender();

    let account = Account { id, owner };

    transfer::share_object(account);
}

/// Immutably borrows the account-owned asset.
public fun asset<Value: key>(self: &Account, asset: address): &Value {

    // TODO: Ensure the account is unlocked before borrowing the asset.

    borrow_asset<Value>(self.owner, asset)
}

/// Tests the `asset` function by borrowing a `Coin` from the account.
public fun test(self: &Account, asset: address) {
    let coin = asset<Coin<IOTA>>(self, asset);

    assert!(coin.get_address() == asset);
    assert!(coin.value() > 0);
}

/// Immutably borrows the account-owned asset.
///
/// The asset type must has the `key` ability to be accessed from the store.
///
/// # Errors
/// - `EAssetDoesNotExist` if the asset does not exist.
/// - `EAssetTypeMismatch` if the type does not match.
/// - `EBCSSerializationFailure` if serialization fails.
native fun borrow_asset<Value: key>(account: address, asset: address): &Value;

// TODO: More assets-related functions can be added here, such as `borrow_mut`, `exists. `remove`, etc.
