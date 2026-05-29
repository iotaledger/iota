// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// This module defines the storage primitives used by `whitelist_sponsorship_account`'s
/// sponsor authenticator:
///
/// - An authenticator functions whitelist (`Bag` keyed by `AuthenticatorFunctionKey`): the set of
///   sender authenticator functions whose transactions a sponsor is willing to pay for. Each
///   entry stores an `AuthenticatorFunctionRefV1<T>` indexed by its `(package, module, function)`
///   identity. Using a `Bag` lets the sponsor whitelist functions for senders of different account
///   types `T`.
/// - A user gas allowances table (`Table<address, u64>`): the maximum gas budget the sponsor
///   will cover, per sponsored user.
///
/// The authenticator logic and PTB-callable allowance mutation that consult these whitelists live
/// in `whitelist_sponsorship_account`. This module owns the dynamic-field layout and the
/// administrative mutations.
module whitelist_sponsorship::whitelists;

use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::bag::{Self, Bag};
use iota::dynamic_field as df;
use iota::table::{Self, Table};
use std::ascii;

// === Errors ===

#[error(code = 0)]
const EWhitelistsAlreadyAttached: vector<u8> = b"Sponsorship whitelists already attached.";

#[error(code = 1)]
const EWhitelistsMissing: vector<u8> = b"Sponsorship whitelists missing.";

#[error(code = 2)]
const EAuthenticatorFunctionAlreadyWhitelisted: vector<u8> =
    b"Authenticator function already whitelisted.";

#[error(code = 3)]
const EAuthenticatorFunctionNotWhitelisted: vector<u8> = b"Authenticator function not whitelisted.";

#[error(code = 4)]
const EUserGasAllowanceAlreadyExists: vector<u8> = b"User gas allowance already exists.";

#[error(code = 5)]
const EUserGasAllowanceMissing: vector<u8> = b"User gas allowance missing.";

// === Structs ===

/// A type-erased identity of an authenticator function `(package, module, function)`. Used as
/// the `Bag` key for the whitelist so that entries with different `T` parameters share the same
/// lookup space.
public struct AuthenticatorFunctionKey has copy, drop, store {
    package: ID,
    module_name: ascii::String,
    function_name: ascii::String,
}

/// A dynamic field name for the whitelist of accepted sender authenticator functions.
public struct AuthenticatorFunctionsWhitelistFieldName has copy, drop, store {}

/// A dynamic field name for the per-user sponsored gas allowance table.
public struct UserGasAllowancesFieldName has copy, drop, store {}

// === Public Functions ===

/// Constructs an `AuthenticatorFunctionKey` from its components.
public fun new_authenticator_function_key(
    package: ID,
    module_name: ascii::String,
    function_name: ascii::String,
): AuthenticatorFunctionKey {
    AuthenticatorFunctionKey { package, module_name, function_name }
}

// === View Functions ===

/// Returns true if both whitelist dynamic fields are attached to the account.
public fun has_whitelists(account_id: &UID): bool {
    df::exists_(account_id, AuthenticatorFunctionsWhitelistFieldName {})
        && df::exists_(account_id, UserGasAllowancesFieldName {})
}

/// Borrows the bag of accepted sender authenticator functions.
public fun borrow_authenticator_functions_whitelist(account_id: &UID): &Bag {
    df::borrow(account_id, AuthenticatorFunctionsWhitelistFieldName {})
}

/// Borrows the table of per-user gas allowances.
public fun borrow_user_gas_allowances(account_id: &UID): &Table<address, u64> {
    df::borrow(account_id, UserGasAllowancesFieldName {})
}

// === Admin Functions ===

/// Attach the (initially empty) authenticator functions whitelist and user gas allowances table
/// to the account.
public(package) fun attach_whitelists(account_id: &mut UID, ctx: &mut TxContext) {
    assert!(!has_whitelists(account_id), EWhitelistsAlreadyAttached);

    df::add(account_id, AuthenticatorFunctionsWhitelistFieldName {}, bag::new(ctx));
    df::add(account_id, UserGasAllowancesFieldName {}, table::new<address, u64>(ctx));
}

/// Detach the whitelists from the account. Both the bag and the user gas allowances table
/// must be empty.
public(package) fun detach_whitelists(account_id: &mut UID) {
    assert!(has_whitelists(account_id), EWhitelistsMissing);

    let whitelist: Bag = df::remove(account_id, AuthenticatorFunctionsWhitelistFieldName {});
    whitelist.destroy_empty();
    let allowances: Table<address, u64> = df::remove(account_id, UserGasAllowancesFieldName {});
    allowances.destroy_empty();
}

/// Adds an authenticator function to the whitelist. Fails if already present.
public(package) fun add_authenticator_function<T: key>(
    account_id: &mut UID,
    auth_fn: AuthenticatorFunctionRefV1<T>,
) {
    assert!(has_whitelists(account_id), EWhitelistsMissing);

    let key = key_from_ref(&auth_fn);
    let whitelist = borrow_mut_authenticator_functions_whitelist(account_id);
    assert!(!whitelist.contains(key), EAuthenticatorFunctionAlreadyWhitelisted);
    whitelist.add(key, auth_fn);
}

/// Removes an authenticator function from the whitelist. Fails if not present. The caller must
/// supply the original `T` so the typed ref can be returned and dropped.
public(package) fun remove_authenticator_function<T: key>(
    account_id: &mut UID,
    auth_fn: &AuthenticatorFunctionRefV1<T>,
) {
    assert!(has_whitelists(account_id), EWhitelistsMissing);

    let key = key_from_ref(auth_fn);
    let whitelist = borrow_mut_authenticator_functions_whitelist(account_id);
    assert!(whitelist.contains(key), EAuthenticatorFunctionNotWhitelisted);
    let _: AuthenticatorFunctionRefV1<T> = whitelist.remove(key);
}

/// Sets the maximum gas budget the sponsor will cover for `user`. Fails if an allowance is
/// already set for `user`.
public(package) fun add_user_gas_allowance(account_id: &mut UID, user: address, allowance: u64) {
    assert!(has_whitelists(account_id), EWhitelistsMissing);

    let allowances = borrow_mut_user_gas_allowances(account_id);
    assert!(!allowances.contains(user), EUserGasAllowanceAlreadyExists);
    allowances.add(user, allowance);
}

/// Updates `user`'s gas allowance and returns the previous one. Fails if no allowance is set.
public(package) fun rotate_user_gas_allowance(
    account_id: &mut UID,
    user: address,
    allowance: u64,
): u64 {
    assert!(has_whitelists(account_id), EWhitelistsMissing);

    let allowances = borrow_mut_user_gas_allowances(account_id);
    assert!(allowances.contains(user), EUserGasAllowanceMissing);
    let prev = allowances.remove(user);
    allowances.add(user, allowance);
    prev
}

/// Removes `user`'s gas allowance and returns the previous value. Fails if no allowance is set.
public(package) fun remove_user_gas_allowance(account_id: &mut UID, user: address): u64 {
    assert!(has_whitelists(account_id), EWhitelistsMissing);

    let allowances = borrow_mut_user_gas_allowances(account_id);
    assert!(allowances.contains(user), EUserGasAllowanceMissing);
    allowances.remove(user)
}

// === Package Functions ===

/// Returns a mutable reference to the bag of accepted sender authenticator functions.
public(package) fun borrow_mut_authenticator_functions_whitelist(account_id: &mut UID): &mut Bag {
    df::borrow_mut(account_id, AuthenticatorFunctionsWhitelistFieldName {})
}

/// Returns a mutable reference to the table of per-user gas allowances.
public(package) fun borrow_mut_user_gas_allowances(account_id: &mut UID): &mut Table<address, u64> {
    df::borrow_mut(account_id, UserGasAllowancesFieldName {})
}

// === Private Functions ===

/// Derives an `AuthenticatorFunctionKey` from an `AuthenticatorFunctionRefV1<T>`.
fun key_from_ref<T: key>(auth_fn: &AuthenticatorFunctionRefV1<T>): AuthenticatorFunctionKey {
    AuthenticatorFunctionKey {
        package: auth_fn.package(),
        module_name: *auth_fn.module_name(),
        function_name: *auth_fn.function_name(),
    }
}

// === Test Functions ===
