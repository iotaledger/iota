// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
module spending_limit::spending_limit;

use iota::balance::{Self, Balance};
use iota::coin::{Self, Coin};
use iota::ed25519;
use iota::iota::IOTA;
use iota::auth_context::AuthContext;
use iota::account::AuthenticatorInfoV1;
use account_template::account_template::{Self, IOTAccount};

#[error(code = 0)]
const EOverLimit: vector<u8> = b"Spending limit exceeded.";

#[error(code = 1)]
const EEd25519VerificationFailed: vector<u8> = b"Ed25519 authenticator verification failed.";

#[error(code = 2)]
const EInsufficientGasReserve: vector<u8> = b"Insufficient gas reserve balance.";

#[error(code = 3)]
const EGasReserveAlreadyInitialized: vector<u8> = b"Gas reserve has already been initialized.";

/// Marker for the account reserved field that stores the spending-limit state.
public struct SpendingStateKey has copy, drop, store {}

/// Marker for the account reserved field that stores the owner's Ed25519 public key.
public struct OwnerPublicKey has copy, drop, store {}

/// Marker for the gas reserve balance (outside spending limit).
public struct GasReserveKey has copy, drop, store {}

/// Per-epoch spending limit state stored on the account.
public struct AuthenticatorWithSpendingLimit has copy, drop, store {
    /// The maximum IOTA amount allowed to be spent per epoch (excluding gas).
    limit: u64,
    /// The amount already used within the current epoch (excluding gas).
    used: u64,
    /// The epoch in which `used` was last updated.
    epoch: option::Option<u64>,
}

/// Separate gas reserve balance that doesn't count toward spending limit.
/// This ensures users can always pay for transaction fees even at the limit.
public struct GasReserve has store {
    balance: Balance<IOTA>,
}

/// A Move-friendly summary of relevant transaction effects for this account.
/// In practice, operational_spend tracks non-gas spending that counts toward the limit.
public struct TxEffects has drop {
    /// Amount spent on actual operations (transfers, purchases, etc.) that counts toward limit.
    operational_spend: u64,
}

public fun new_tx_effects(operational_spend: u64): TxEffects {
    TxEffects { operational_spend }
}

/// Helper accessor for operational spending (what counts toward limit).
public fun operational_spend(effects: &TxEffects): u64 {
    effects.operational_spend
}

/// Create a new account with spending limit and separate gas reserve.
///
/// # Arguments
/// * `limit` - Per-epoch spending limit for operational transactions (excluding gas)
/// * `initial_gas_reserve` - Initial IOTA balance for gas payments
/// * `public_key` - Ed25519 public key for authentication
/// * `authenticator` - Authenticator configuration
public fun create(
    limit: u64,
    initial_gas_reserve: Coin<IOTA>,  // <- Pass gas during creation
    public_key: vector<u8>,
    authenticator: AuthenticatorInfoV1,
    ctx: &mut TxContext,
) {
    let mut builder = account_template::init_account_builder(ctx);
    builder.set_authenticator(authenticator);

    // Initialize spending state
    builder.add_reserved_field(
        SpendingStateKey {},
        AuthenticatorWithSpendingLimit {
            limit,
            used: 0,
            epoch: option::none(),
        }
    );

    builder.add_reserved_field(OwnerPublicKey {}, public_key);
    
    // Add gas reserve as a reserved field
    builder.add_reserved_field(
        GasReserveKey {},
        GasReserve {
            balance: coin::into_balance(initial_gas_reserve),
        }
    );

    builder.finish_and_share();
}

/// Must be called after account creation to initialize the gas reserve.
/// This is a separate transaction because the account must be shared first.
public fun initialize_gas_reserve(
    self: &mut IOTAccount,
    initial_gas_reserve: Coin<IOTA>,
    ctx: &TxContext,
) {
    account_template::ensure_tx_sender_is_account(self, ctx);
    
    // Verify gas reserve doesn't already exist
    assert!(!self.has_field(GasReserveKey {}), EGasReserveAlreadyInitialized);
    
    // Add as non-reserved field so it can be modified later
    self.add_field(
        GasReserveKey {},
        GasReserve {
            balance: coin::into_balance(initial_gas_reserve),
        },
        ctx,
    );
}

/// Authenticate access for the spending-limited account.
/// Validates transaction sender and Ed25519 signature.
public fun authenticate(
    self: &IOTAccount,
    signature: vector<u8>,
    _auth_ctx: &AuthContext,
    ctx: &TxContext,
) {
    account_template::ensure_tx_sender_is_account(self, ctx);

    let public_key: &vector<u8> = self.borrow_field(OwnerPublicKey {});
    assert!(
        ed25519::ed25519_verify(&signature, public_key, ctx.digest()),
        EEd25519VerificationFailed,
    );
}

/// Pre-execution capacity check with safety margin.
/// Clients MUST call this before submitting transactions near the limit.
///
/// # Arguments
/// * `estimated_operational_spend` - Estimated non-gas spending for the transaction
/// * `safety_margin_percent` - Safety margin percentage (e.g., 20 for 20%)
///
/// # Returns
/// `true` if the transaction is likely to succeed, `false` otherwise
public fun check_spending_capacity(
    self: &IOTAccount,
    estimated_operational_spend: u64,
    safety_margin_percent: u64,
    ctx: &TxContext,
): bool {
    let state: &AuthenticatorWithSpendingLimit = self.borrow_field(SpendingStateKey {});
    let epoch_now = ctx.epoch();
    
    // Calculate current usage for this epoch.
    // If epoch changed, usage resets to 0.
    // 
    let current_used = if (option::is_some(&state.epoch)) {
        let prev_epoch = option::borrow(&state.epoch);
        if (*prev_epoch == epoch_now) state.used else 0
    } else { 0 };
    
    // Add safety margin to account for estimation errors.
    let margin = (estimated_operational_spend * safety_margin_percent) / 100;
    let estimated_with_margin = estimated_operational_spend + margin;

    // Return whether the transaction is likely to succeed.
    current_used + estimated_with_margin <= state.limit
}

/// Post-execution hook to enforce per-epoch spending limits.
/// Only operational spending counts toward the limit; gas fees are paid separately via
/// the gas payment mechanism using coins from the gas reserve.
///
/// # Important
/// This function must be called immediately after transaction execution with accurate `TxEffects`.
/// Gas payment itself is handled by the IOTA transaction system, not by this module.
public fun post_execution(
    self: &mut IOTAccount,
    effects: &TxEffects,
    ctx: &TxContext,
) {
    let state: &AuthenticatorWithSpendingLimit = self.borrow_field(SpendingStateKey {});
    let epoch_now = ctx.epoch();
    let mut new_used = state.used;

    // Reset usage if epoch changed.
    if (option::is_some(&state.epoch)) {
        let prev_epoch_ref = option::borrow(&state.epoch);
        if (*prev_epoch_ref != epoch_now) {
            new_used = 0;
        };
    } else {
        new_used = 0;
    };

    // Add operational spending to usage.
    new_used = new_used + effects.operational_spend;

    // Enforce the per-epoch limit.
    assert!(new_used <= state.limit, EOverLimit);

    // Rotate the reserved field with updated state.
    let new_state = AuthenticatorWithSpendingLimit {
        limit: state.limit,
        used: new_used,
        epoch: option::some(epoch_now),
    };
    
    self.rotate_reserved(SpendingStateKey {}, new_state, ctx);
}

// --------------------------------------- Gas Reserve Management ---------------------------------------

/// Deposit IOTA into the gas reserve.
/// This does not count toward the spending limit.
/// Users should maintain sufficient gas reserve to pay for transaction fees.
public fun deposit_to_gas_reserve(
    self: &mut IOTAccount,
    additional: Coin<IOTA>,
    ctx: &TxContext,
) {
    account_template::ensure_tx_sender_is_account(self, ctx);
    
    let reserve = account_template::borrow_field_mut<GasReserveKey, GasReserve>(
        self,
        GasReserveKey {},
        ctx
    );
    
    balance::join(&mut reserve.balance, coin::into_balance(additional));
}


/// Withdraw IOTA from the gas reserve as a payment coin.
/// This is typically used to extract a coin for gas payment in transactions.
/// Only the account owner can withdraw.
public fun withdraw_from_gas_reserve(
    self: &mut IOTAccount,
    amount: u64,
    ctx: &mut TxContext,
): Coin<IOTA> {
    account_template::ensure_tx_sender_is_account(self, ctx);
    
    let reserve: &mut GasReserve = self.borrow_field_mut(GasReserveKey {}, ctx);
    assert!(balance::value(&reserve.balance) >= amount, EInsufficientGasReserve);
    
    let withdrawn_balance = balance::split(&mut reserve.balance, amount);
    coin::from_balance(withdrawn_balance, ctx)
}

/// Get a gas payment coin from the reserve.
/// This is the primary way to obtain IOTA for transaction gas payment.
/// The coin should be used in the `gas_payment` field of the transaction.
public fun get_gas_coin_from_reserve(
    self: &mut IOTAccount,
    amount: u64,
    ctx: &mut TxContext,
): Coin<IOTA> {
    withdraw_from_gas_reserve(self, amount, ctx)
}

/// Get the current gas reserve balance.
public fun get_gas_reserve_balance(self: &IOTAccount): u64 {
    if (self.has_field(GasReserveKey {})) {
        let reserve: &GasReserve = self.borrow_field(GasReserveKey {});
        balance::value(&reserve.balance)
    } else {
        0
    }
}

/// Get the current spending state (for monitoring/UI purposes).
/// Returns (limit, used, epoch).
public fun get_spending_state(self: &IOTAccount): (u64, u64, option::Option<u64>) {
    let state: &AuthenticatorWithSpendingLimit = self.borrow_field(SpendingStateKey {});
    (state.limit, state.used, state.epoch)
}

/// Get remaining spending capacity for the current epoch.
public fun get_remaining_capacity(self: &IOTAccount, ctx: &TxContext): u64 {
    let state: &AuthenticatorWithSpendingLimit = self.borrow_field(SpendingStateKey {});
    let epoch_now = ctx.epoch();
    
    let current_used = if (option::is_some(&state.epoch)) {
        let prev_epoch = option::borrow(&state.epoch);
        if (*prev_epoch == epoch_now) state.used else 0
    } else { 0 };
    
    if (state.limit >= current_used) {
        state.limit - current_used
    } else {
        0
    }
}

// --------------------------------------- Test Utilities ---------------------------------------

#[test_only]
public fun create_owner_public_key_for_testing(): OwnerPublicKey {
    OwnerPublicKey {}
}

#[test_only]
public fun create_spending_state_key_for_testing(): SpendingStateKey {
    SpendingStateKey {}
}

#[test_only]
public fun create_gas_reserve_key_for_testing(): GasReserveKey {
    GasReserveKey {}
}

#[test_only]
public fun test_state(limit: u64): AuthenticatorWithSpendingLimit {
    AuthenticatorWithSpendingLimit {
        limit,
        used: 0,
        epoch: option::none(),
    }
}
