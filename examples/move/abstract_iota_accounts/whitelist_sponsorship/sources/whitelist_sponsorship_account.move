// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// This module defines a `WhitelistSponsorshipAccount` whose authenticator allows the account
/// to act as a sponsor for transactions whose sender authenticator function and gas budget are
/// within the per-account whitelists maintained by `whitelists`.
///
/// The `authenticator` function enforces both whitelists by inspecting the sender's authenticator
/// function via the `AuthContext` and the transaction gas budget via the `TxContext`. Because
/// authenticators cannot mutate state, the sender is required to include a PTB command that calls
/// `deduct_user_gas_allowance` on this account to pay back the gas budget against their allowance;
/// `authenticator` scans the PTB to verify this.
///
/// Administration of the whitelists is gated by a separate admin address stored on the account.
module whitelist_sponsorship::whitelist_sponsorship_account;

use iota::account;
use iota::address;
use iota::auth_context::AuthenticatorFunctionInfoV1;
use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::bag::Bag;
use iota::bcs;
use iota::dynamic_field as df;
use iota::ptb_call_arg::CallArg;
use iota::ptb_command::{Argument, ProgrammableMoveCall};
use iota::table::Table;
use std::ascii;
use std::type_name;
use whitelist_sponsorship::whitelists::{Self, AuthenticatorFunctionKey};

/// Method-syntax alias for `ptb_command::function`, which clashes with the `function_name`
/// accessor on `AuthenticatorFunctionKey`.
use fun iota::ptb_command::function as ProgrammableMoveCall.move_call_function;

/// Method-syntax alias for the deduct-call recognizer.
use fun is_deduct_call as ProgrammableMoveCall.is_deduct_call;

/// Method-syntax alias for the sponsor-first-arg check.
use fun first_arg_is_sponsor as ProgrammableMoveCall.first_arg_is_sponsor;

// === Errors ===

#[error(code = 0)]
const ENotAdmin: vector<u8> = b"Sender is not the admin of this account.";

#[error(code = 1)]
const EWhitelistsMissing: vector<u8> = b"Sponsorship whitelists missing.";

#[error(code = 2)]
const ENotASponsoredTransaction: vector<u8> = b"Transaction is not sponsored by this account.";

#[error(code = 3)]
const ESenderAuthenticatorFunctionMissing: vector<u8> = b"Sender does not use a MoveAuthenticator.";

#[error(code = 4)]
const EAuthenticatorFunctionNotWhitelisted: vector<u8> = b"Authenticator function not whitelisted.";

#[error(code = 5)]
const EUserGasAllowanceMissing: vector<u8> = b"User gas allowance missing.";

#[error(code = 6)]
const EGasBudgetExceedsAllowance: vector<u8> =
    b"Transaction gas budget exceeds the sponsored user's allowance.";

#[error(code = 7)]
const EInsufficientAllowanceDeducted: vector<u8> =
    b"PTB does not deduct enough allowance to cover the gas budget.";

#[error(code = 8)]
const EInvalidDeductCall: vector<u8> = b"Invalid deduct allowance call in PTB.";

#[error(code = 9)]
const EAdminMissing: vector<u8> = b"Admin is not set on this account.";

#[error(code = 10)]
const ENotUserOrAdmin: vector<u8> = b"Sender is not the user or the admin.";

#[error(code = 11)]
const EInsufficientAllowanceForDeduction: vector<u8> =
    b"Allowance is insufficient for the deducted amount.";

// === Constants ===

/// The function name of `deduct_user_gas_allowance` in this module, used by the PTB scan.
const DEDUCT_USER_GAS_ALLOWANCE_FUNC_NAME: vector<u8> = b"deduct_user_gas_allowance";

// === Structs ===

/// A sponsoring account whose authenticator enforces whitelists of accepted sender
/// authenticator functions and per-user gas allowances.
public struct WhitelistSponsorshipAccount has key {
    id: UID,
}

/// Dynamic field name for the account admin address.
public struct AdminFieldName has copy, drop, store {}

// === Account Helpers ===

/// Creates a new `WhitelistSponsorshipAccount` as a shared object. Attaches the whitelists,
/// the admin address, and the given authenticator.
public fun create(
    admin: address,
    authenticator: AuthenticatorFunctionRefV1<WhitelistSponsorshipAccount>,
    ctx: &mut TxContext,
) {
    let mut sponsorship_account = WhitelistSponsorshipAccount { id: object::new(ctx) };
    let id = &mut sponsorship_account.id;

    whitelists::attach_whitelists(id, ctx);
    df::add(id, AdminFieldName {}, admin);

    account::create_account_v1(sponsorship_account, authenticator);
}

/// Deducts `amount` from `user`'s gas allowance on this sponsor account. Intended to be called
/// from the sender's PTB during a sponsored transaction so the sender's allowance is reduced by
/// the gas budget. `authenticator` scans the PTB for calls to this exact function.
///
/// Only the `user` themselves or the admin can call this.
public fun deduct_user_gas_allowance(
    self: &mut WhitelistSponsorshipAccount,
    user: address,
    amount: u64,
    ctx: &TxContext,
) {
    assert!(ctx.sender() == user || ctx.sender() == self.borrow_admin(), ENotUserOrAdmin);
    assert!(whitelists::has_whitelists(&self.id), EWhitelistsMissing);

    let allowances = whitelists::borrow_mut_user_gas_allowances(&mut self.id);
    assert!(allowances.contains(user), EUserGasAllowanceMissing);
    let entry = allowances.borrow_mut(user);
    assert!(*entry >= amount, EInsufficientAllowanceForDeduction);
    *entry = *entry - amount;
}

// === Authenticators ===

/// Authenticator for `WhitelistSponsorshipAccount`.
///
/// Aborts if:
/// - the whitelists are not attached to the account,
/// - the transaction is not sponsored by this account,
/// - the sender does not use a `MoveAuthenticator`,
/// - the sender's authenticator function is not in the whitelist,
/// - the sender has no gas allowance,
/// - the transaction gas budget exceeds the sender's allowance,
/// - the PTB does not deduct at least the gas budget from the sender's allowance.
#[authenticator]
public fun authenticator(
    account: &WhitelistSponsorshipAccount,
    auth_ctx: &AuthContext,
    ctx: &TxContext,
) {
    let account_id = account.borrow_uid();

    // Check if the account was setup.
    assert!(whitelists::has_whitelists(account_id), EWhitelistsMissing);

    // Check if the transaction is sponsored by this account.
    let sponsor_opt: Option<address> = ctx.sponsor();
    assert!(sponsor_opt.is_some(), ENotASponsoredTransaction);
    assert!(sponsor_opt.destroy_some() == account_id.to_address(), ENotASponsoredTransaction);

    // Check that the sender uses a `MoveAuthenticator` whose function is in the whitelist.
    let sender_info_opt = auth_ctx.sender_authenticator_function_info_v1();
    assert!(sender_info_opt.is_some(), ESenderAuthenticatorFunctionMissing);
    let key = key_from_info(sender_info_opt.borrow());
    let whitelist = whitelists::borrow_authenticator_functions_whitelist(
        account_id,
    );
    assert!(whitelist.contains(key), EAuthenticatorFunctionNotWhitelisted);

    // Check that the transaction gas budget fits within the sender's allowance.
    let sender = ctx.sender();
    let allowances = whitelists::borrow_user_gas_allowances(account_id);
    assert!(allowances.contains(sender), EUserGasAllowanceMissing);
    assert!(ctx.gas_budget() <= *allowances.borrow(sender), EGasBudgetExceedsAllowance);

    // Finally, check that the sender used a command in the PTB to deduct the allowance from the sponsor's account.
    // This would be called directly in this authenticator if authenticators were allowed to modify the state.
    let deducted = lookup_and_calculate_deductions(account_id, sender, auth_ctx);
    assert!(deducted >= ctx.gas_budget(), EInsufficientAllowanceDeducted);
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

/// Returns the admin address. Aborts with `EAdminMissing` if no admin is set.
public fun borrow_admin(self: &WhitelistSponsorshipAccount): address {
    assert!(df::exists_(&self.id, AdminFieldName {}), EAdminMissing);
    *df::borrow(&self.id, AdminFieldName {})
}

/// Returns true if the whitelists are attached.
public fun has_whitelists(account: &WhitelistSponsorshipAccount): bool {
    whitelists::has_whitelists(account.borrow_uid())
}

/// Borrows the bag of accepted sender authenticator functions.
public fun borrow_authenticator_functions_whitelist(account: &WhitelistSponsorshipAccount): &Bag {
    whitelists::borrow_authenticator_functions_whitelist(account.borrow_uid())
}

/// Borrows the table of per-user gas allowances.
public fun borrow_user_gas_allowances(account: &WhitelistSponsorshipAccount): &Table<address, u64> {
    whitelists::borrow_user_gas_allowances(account.borrow_uid())
}

// === Admin Functions ===

/// Attach the (initially empty) whitelists to the account. Only the admin can call this.
public fun attach_whitelists(self: &mut WhitelistSponsorshipAccount, ctx: &mut TxContext) {
    assert!(ctx.sender() == self.borrow_admin(), ENotAdmin);
    whitelists::attach_whitelists(&mut self.id, ctx);
}

/// Detach the whitelists from the account. Only the admin can call this.
public fun detach_whitelists(self: &mut WhitelistSponsorshipAccount, ctx: &TxContext) {
    assert!(ctx.sender() == self.borrow_admin(), ENotAdmin);
    whitelists::detach_whitelists(&mut self.id);
}

/// Adds an authenticator function to the whitelist. Only the admin can call this.
public fun add_authenticator_function<T: key>(
    self: &mut WhitelistSponsorshipAccount,
    auth_fn: AuthenticatorFunctionRefV1<T>,
    ctx: &TxContext,
) {
    assert!(ctx.sender() == self.borrow_admin(), ENotAdmin);
    whitelists::add_authenticator_function(&mut self.id, auth_fn);
}

/// Removes an authenticator function from the whitelist. Only the admin can call this.
public fun remove_authenticator_function<T: key>(
    self: &mut WhitelistSponsorshipAccount,
    auth_fn: &AuthenticatorFunctionRefV1<T>,
    ctx: &TxContext,
) {
    assert!(ctx.sender() == self.borrow_admin(), ENotAdmin);
    whitelists::remove_authenticator_function(&mut self.id, auth_fn);
}

/// Sets the maximum gas budget the sponsor will cover for `user`. Only the admin can call this.
public fun add_user_gas_allowance(
    self: &mut WhitelistSponsorshipAccount,
    user: address,
    allowance: u64,
    ctx: &TxContext,
) {
    assert!(ctx.sender() == self.borrow_admin(), ENotAdmin);
    whitelists::add_user_gas_allowance(&mut self.id, user, allowance);
}

/// Updates `user`'s gas allowance and returns the previous one. Only the admin can call this.
public fun rotate_user_gas_allowance(
    self: &mut WhitelistSponsorshipAccount,
    user: address,
    allowance: u64,
    ctx: &TxContext,
): u64 {
    assert!(ctx.sender() == self.borrow_admin(), ENotAdmin);
    whitelists::rotate_user_gas_allowance(
        &mut self.id,
        user,
        allowance,
    )
}

/// Removes `user`'s gas allowance and returns the previous value. Only the admin can call this.
public fun remove_user_gas_allowance(
    self: &mut WhitelistSponsorshipAccount,
    user: address,
    ctx: &TxContext,
): u64 {
    assert!(ctx.sender() == self.borrow_admin(), ENotAdmin);
    whitelists::remove_user_gas_allowance(&mut self.id, user)
}

// === Package Functions ===

// === Private Functions ===

/// Derives an `AuthenticatorFunctionKey` from the framework's type-erased
/// `AuthenticatorFunctionInfoV1` returned by `AuthContext`.
fun key_from_info(info: &AuthenticatorFunctionInfoV1): AuthenticatorFunctionKey {
    whitelists::new_authenticator_function_key(
        info.package(),
        *info.module_name(),
        *info.function_name(),
    )
}

/// Scans the PTB commands for calls to `deduct_user_gas_allowance` on this sponsor account and
/// `sender`, summing the deducted amounts. The expected call signature is
/// `whitelist_sponsorship_account::deduct_user_gas_allowance(&mut WhitelistSponsorshipAccount, user, amount)`.
fun lookup_and_calculate_deductions(
    account_id: &UID,
    sender: address,
    auth_ctx: &AuthContext,
): u64 {
    let commands = auth_ctx.tx_commands();
    let inputs = auth_ctx.tx_inputs();
    let sponsor_address = account_id.to_address();

    let mut total = 0u64;

    commands.do_ref!(|command| {
        command
            .as_move_call()
            .do!(
                |call| if (
                    call.is_deduct_call() && call.first_arg_is_sponsor(inputs, sponsor_address)
                ) {
                    // Args: [sponsor_account, user, amount].
                    let args = call.arguments();
                    assert!(args.length() == 3, EInvalidDeductCall);

                    // The user argument must be a pure address equal to the sender.
                    let user_addr = pure_address_at(inputs, &args[1]);
                    assert!(user_addr == sender, EInvalidDeductCall);

                    // The amount argument must be a pure u64.
                    let amount = pure_u64_at(inputs, &args[2]);
                    total = total + amount;
                },
            );
    });

    total
}

/// Returns true if `call` is a call to `deduct_user_gas_allowance` in this module.
fun is_deduct_call(call: &ProgrammableMoveCall): bool {
    let self_type = type_name::get<WhitelistSponsorshipAccount>();
    if (
        call.move_call_function() != &ascii::string(DEDUCT_USER_GAS_ALLOWANCE_FUNC_NAME)
        || call.module_name() != &self_type.get_module()
    ) {
        return false
    };

    let call_package_addr = object::id_to_address(call.package());
    let expected_package_addr = address::from_ascii_bytes(self_type.get_address().as_bytes());

    call_package_addr == expected_package_addr
}

/// Returns true if the first argument of `call` is an object whose id equals `sponsor`.
fun first_arg_is_sponsor(
    call: &ProgrammableMoveCall,
    inputs: &vector<CallArg>,
    sponsor: address,
): bool {
    let args = call.arguments();
    if (args.is_empty()) return false;

    let input_ix_opt = args[0].input_index();
    if (input_ix_opt.is_none()) return false;
    let input_ix = input_ix_opt.destroy_some() as u64;
    if (input_ix >= inputs.length()) return false;

    let call_arg = &inputs[input_ix];
    if (call_arg.is_pure_data()) return false;

    let obj_data = call_arg.as_object_data().destroy_some();
    let obj_id_opt = obj_data.object_id();
    if (obj_id_opt.is_none()) return false;

    object::id_to_address(&obj_id_opt.destroy_some()) == sponsor
}

/// Reads a pure `address` from the PTB input pointed to by `arg`.
fun pure_address_at(inputs: &vector<CallArg>, arg: &Argument): address {
    let input_idx = arg.input_index().destroy_some() as u64;
    assert!(input_idx < inputs.length(), EInvalidDeductCall);
    let bytes = inputs[input_idx].as_pure_data().destroy_some();
    let mut bcs_stream = bcs::new(bytes);
    bcs_stream.peel_address()
}

/// Reads a pure `u64` from the PTB input pointed to by `arg`.
fun pure_u64_at(inputs: &vector<CallArg>, arg: &Argument): u64 {
    let input_idx = arg.input_index().destroy_some() as u64;
    assert!(input_idx < inputs.length(), EInvalidDeductCall);
    let bytes = inputs[input_idx].as_pure_data().destroy_some();
    let mut bcs_stream = bcs::new(bytes);
    bcs_stream.peel_u64()
}

// === Test Functions ===
