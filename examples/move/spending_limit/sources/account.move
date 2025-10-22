// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module spending_limit::account;

use generic_keyed_authentication::owner_public_key;
use iota::account::{Self, AuthenticatorInfoV1};
use iota::auth_context::AuthContext;
use iota::coin::{Self, Coin};
use iotaccount::iotaccount;
use spending_limit::spending_limit;

// === Errors ===

// === Constants ===

// === Structs ===

public struct SpendLimit has key {
    id: UID,
}

// === Events ===

// === Method Aliases ===

// === Public Functions ===

public fun create(
    public_key: vector<u8>,
    limit: u64,
    authenticator: AuthenticatorInfoV1,
    ctx: &mut TxContext,
) {
    // Attach authenticator info.
    let mut id = object::new(ctx);
    account::attach_auth_info_v1(
        &mut id,
        authenticator,
    );
    // Attach public key using the owner_public_key module.
    owner_public_key::attach(&mut id, public_key);
    // Attach spending limit.
    spending_limit::attach(
        &mut id,
        limit,
    );
    let spend_limit_account = SpendLimit { id };
    iota::transfer::share_object(spend_limit_account);
}

public fun authenticate<T>(
    account: &SpendLimit,
    coins: &vector<Coin<T>>,
    signature: vector<u8>,
    _auth_ctx: &AuthContext,
    ctx: &TxContext,
) {
    iotaccount::ensure_tx_sender_is_account_id(&account.id, ctx);

    owner_public_key::authenticate_ed25519(&account.id, signature, ctx.digest());

    // Calculate actual amount from coin objects - can't be faked
    let actual_amount = calculate_coin_sum(coins);

    spending_limit::authenticate_with_amount(&account.id, actual_amount);
}

// === View Functions ===

// Get the spending limit value.
public fun spending_limit(account: &SpendLimit): u64 {
    *spending_limit::borrow(&account.id)
}

// Query the address of the `SpendLimit` account.
public fun account_address(self: &SpendLimit): address {
    self.id.to_address()
}

// Get the owner public key.
public fun public_key(account: &SpendLimit): &vector<u8> {
    owner_public_key::borrow(&account.id)
}

// Get the authenticator info.
public fun authenticator_info(account: &SpendLimit): &AuthenticatorInfoV1 {
    account::borrow_auth_info_v1(&account.id)
}

// Calculate the sum of coin values.
public fun calculate_coin_sum<T>(coins: &vector<Coin<T>>): u64 {
    let mut total = 0;
    let mut i = 0;
    let len = vector::length(coins);

    while (i < len) {
        let coin = vector::borrow(coins, i);
        total = total + coin::value(coin);
        i = i + 1;
    };

    total
}

// === Admin Functions ===

// === Package Functions ===

// === Private Functions ===

// === Test Functions ===
