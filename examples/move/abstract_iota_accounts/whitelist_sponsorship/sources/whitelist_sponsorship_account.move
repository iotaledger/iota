// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// This module owns the storage and admin surface of `WhitelistSponsorshipAccount` — a sponsor
/// account that gates which sender authenticator functions and which per-user gas budgets it is
/// willing to pay for.
///
/// All policy state lives as inline struct fields so the authenticator hot path (in
/// `whitelist_sponsorship_authentication`) borrows it directly without any extra hop to a
/// dynamic-field-stored container:
/// - The admin address.
/// - A `Table<address, u64>` of per-user gas allowances.
/// - A `Bag` of accepted sender authenticator functions, heterogeneous in the value's `T` so
///   different authenticator-function types share one lookup space.
/// - `package_addr` — the published address of *this* package, captured once at `create` time
///   via `type_name::get<WhitelistSponsorshipAccount>()`. The authenticator reads it on every
///   call to avoid paying for runtime reflection in the hot path.
///
/// `deduct_user_gas_allowance` is callable by the sponsored user from inside their PTB so they
/// can pay back the gas budget the sponsor will spend — the authenticator scans the PTB for
/// exactly this call.
module whitelist_sponsorship::whitelist_sponsorship_account;

use iota::account;
use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::bag::{Self, Bag};
use iota::table::{Self, Table};
use std::ascii;

// === Errors ===

#[error(code = 0)]
const ENotAdmin: vector<u8> = b"Sender is not the admin of this account.";

#[error(code = 1)]
const EUserGasAllowanceMissing: vector<u8> = b"User gas allowance missing.";

#[error(code = 2)]
const EUserGasAllowanceAlreadyExists: vector<u8> = b"User gas allowance already exists.";

#[error(code = 3)]
const EInsufficientAllowanceForDeduction: vector<u8> =
    b"Allowance is insufficient for the deducted amount.";

// === Structs ===

/// A sponsoring account whose authenticator enforces a whitelist of accepted sender
/// authenticator functions and per-user gas allowances. All policy state lives as inline
/// fields so the authenticator hot path borrows it directly: the admin, the per-user gas
/// allowance table, the `Bag` of accepted sender authenticator functions (heterogeneous in
/// `T`, keyed by `AuthenticatorFunctionKey`), and a cached `package_addr` set once at
/// `create` time via `type_name::get<WhitelistSponsorshipAccount>()`.
public struct WhitelistSponsorshipAccount has key {
    id: UID,
    admin: address,
    user_gas_allowances: Table<address, u64>,
    authenticator_functions: Bag,
    package_addr: address,
}

/// A type-erased identity of an authenticator function `(package, module, function)`. Entries
/// with different `T` parameters in the source `AuthenticatorFunctionRefV1<T>` share the same
/// lookup space because the key drops the type parameter.
public struct AuthenticatorFunctionKey has copy, drop, store {
    package: ID,
    module_name: ascii::String,
    function_name: ascii::String,
}

// === Account Helpers ===

/// Creates a new `WhitelistSponsorshipAccount` as a shared object with the given admin and the
/// given sponsor authenticator. The per-user gas allowance table and the authenticator-function
/// whitelist are initialised empty.
public fun create(
    admin: address,
    authenticator: AuthenticatorFunctionRefV1<WhitelistSponsorshipAccount>,
    ctx: &mut TxContext,
) {
    // Compute the package address once, here, via runtime reflection — the authenticator reads
    // this back as `account.package_addr` on every call, avoiding the per-call cost.
    let self_type = std::type_name::get<WhitelistSponsorshipAccount>();
    let package_addr = iota::address::from_ascii_bytes(self_type.get_address().as_bytes());

    let sponsorship_account = WhitelistSponsorshipAccount {
        id: object::new(ctx),
        admin,
        user_gas_allowances: table::new<address, u64>(ctx),
        authenticator_functions: bag::new(ctx),
        package_addr,
    };
    account::create_account_v1(sponsorship_account, authenticator);
}

/// Deducts the transaction's `gas_budget` from the **sender's** gas allowance on this sponsor
/// account. Intended to be called from the sender's PTB during a sponsored transaction so the
/// sender's allowance is reduced by exactly the gas budget the sponsor will pay.
///
/// The sponsor authenticator scans the PTB for the presence of this call. Because the function
/// implicitly targets `ctx.sender()` and always deducts `ctx.gas_budget()`, the authenticator
/// only needs to confirm such a call exists — it doesn't have to read or compare any arguments
/// besides the sponsor account itself.
public fun deduct_user_gas_allowance(self: &mut WhitelistSponsorshipAccount, ctx: &TxContext) {
    let sender = ctx.sender();
    assert!(self.user_gas_allowances.contains(sender), EUserGasAllowanceMissing);
    let amount = ctx.gas_budget();
    let entry = self.user_gas_allowances.borrow_mut(sender);
    assert!(*entry >= amount, EInsufficientAllowanceForDeduction);
    *entry = *entry - amount;
}

// === View Functions ===

/// Returns the account's UID.
public fun borrow_uid(self: &WhitelistSponsorshipAccount): &UID {
    &self.id
}

/// Returns the account's address.
public fun account_address(self: &WhitelistSponsorshipAccount): address {
    self.id.to_address()
}

/// Returns the admin address.
public fun borrow_admin(self: &WhitelistSponsorshipAccount): address {
    self.admin
}

/// Returns the cached package address (the published address of this package, captured at
/// `create` time via `type_name::get<WhitelistSponsorshipAccount>()`). The authenticator reads
/// this on every call to avoid the per-authenticator reflection cost.
public fun borrow_package_addr(self: &WhitelistSponsorshipAccount): address {
    self.package_addr
}

/// Returns true if `key` names an accepted sender authenticator function for this account.
public fun is_authenticator_function_whitelisted(
    account: &WhitelistSponsorshipAccount,
    key: AuthenticatorFunctionKey,
): bool {
    account.authenticator_functions.contains(key)
}

/// Borrows the bag of accepted sender authenticator functions.
public fun borrow_authenticator_functions(account: &WhitelistSponsorshipAccount): &Bag {
    &account.authenticator_functions
}

/// Borrows the table of per-user gas allowances.
public fun borrow_user_gas_allowances(account: &WhitelistSponsorshipAccount): &Table<address, u64> {
    &account.user_gas_allowances
}

/// Constructs an `AuthenticatorFunctionKey` from its components.
public fun new_authenticator_function_key(
    package: ID,
    module_name: ascii::String,
    function_name: ascii::String,
): AuthenticatorFunctionKey {
    AuthenticatorFunctionKey { package, module_name, function_name }
}

// === Admin Functions ===

/// Adds an authenticator function to the whitelist. Only the admin can call this.
public fun add_authenticator_function<T: key>(
    self: &mut WhitelistSponsorshipAccount,
    auth_fn: AuthenticatorFunctionRefV1<T>,
    ctx: &TxContext,
) {
    assert!(ctx.sender() == self.admin, ENotAdmin);
    let key = key_from_ref(&auth_fn);
    self.authenticator_functions.add(key, auth_fn);
}

/// Removes an authenticator function from the whitelist. Only the admin can call this.
public fun remove_authenticator_function<T: key>(
    self: &mut WhitelistSponsorshipAccount,
    auth_fn: &AuthenticatorFunctionRefV1<T>,
    ctx: &TxContext,
) {
    assert!(ctx.sender() == self.admin, ENotAdmin);
    let key = key_from_ref(auth_fn);
    let _: AuthenticatorFunctionRefV1<T> = self.authenticator_functions.remove(key);
}

/// Sets the maximum gas budget the sponsor will cover for `user`. Only the admin can call this.
public fun add_user_gas_allowance(
    self: &mut WhitelistSponsorshipAccount,
    user: address,
    allowance: u64,
    ctx: &TxContext,
) {
    assert!(ctx.sender() == self.admin, ENotAdmin);
    assert!(!self.user_gas_allowances.contains(user), EUserGasAllowanceAlreadyExists);
    self.user_gas_allowances.add(user, allowance);
}

/// Updates `user`'s gas allowance and returns the previous one. Only the admin can call this.
public fun rotate_user_gas_allowance(
    self: &mut WhitelistSponsorshipAccount,
    user: address,
    allowance: u64,
    ctx: &TxContext,
): u64 {
    assert!(ctx.sender() == self.admin, ENotAdmin);
    assert!(self.user_gas_allowances.contains(user), EUserGasAllowanceMissing);
    let prev = self.user_gas_allowances.remove(user);
    self.user_gas_allowances.add(user, allowance);
    prev
}

/// Removes `user`'s gas allowance and returns the previous value. Only the admin can call this.
public fun remove_user_gas_allowance(
    self: &mut WhitelistSponsorshipAccount,
    user: address,
    ctx: &TxContext,
): u64 {
    assert!(ctx.sender() == self.admin, ENotAdmin);
    assert!(self.user_gas_allowances.contains(user), EUserGasAllowanceMissing);
    self.user_gas_allowances.remove(user)
}

/// Admin-side deduction: subtracts `amount` from `user`'s gas allowance without going through
/// the sponsored-transaction flow. Useful for out-of-band rebalancing — not scanned by the
/// authenticator and so does not, on its own, satisfy the sponsor's PTB-deduct requirement.
public fun admin_deduct_user_gas_allowance(
    self: &mut WhitelistSponsorshipAccount,
    user: address,
    amount: u64,
    ctx: &TxContext,
) {
    assert!(ctx.sender() == self.admin, ENotAdmin);
    assert!(self.user_gas_allowances.contains(user), EUserGasAllowanceMissing);
    let entry = self.user_gas_allowances.borrow_mut(user);
    assert!(*entry >= amount, EInsufficientAllowanceForDeduction);
    *entry = *entry - amount;
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
