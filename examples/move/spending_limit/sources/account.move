// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module spending_limit::account;

use generic_keyed_authentication::owner_public_key;
use iota::account::{Self, AuthenticatorInfoV1};
use iota::auth_context::{AuthContext, tx_commands, tx_inputs};
use iota::balance::{Self, Balance};
use iota::bcs;
use iota::coin::{Self, Coin};
use iota::programmable_transaction::{
    move_call_data,
    command_to_int,
    arguments,
    package_id,
    function_name,
    module_name,
    ProgrammableMoveCall,
    argument_input,
    is_object_data,
    is_pure_data,
    is_shared_object,
    shared_object_data
};
use iotaccount::iotaccount;
use spending_limit::spending_limit;
use std::ascii;
use std::type_name::{get, get_address};

// === Errors ===

#[error(code = 0)]
const EInsufficientBalanceReserve: vector<u8> = b"Insufficient balance reserve.";

#[error(code = 1)]
const EUnauthorizedWithdrawCall: vector<u8> = b"Unauthorized withdraw_from_balance_reserve call.";

#[error(code = 2)]
const EInvalidArgumentType: vector<u8> = b"Invalid argument type.";

// === Constants ===

// === Structs ===

public struct SpendLimit has key {
    id: UID,
}

// Marker for the gas reserve balance (outside spending limit).
public struct BalanceReserveKey has copy, drop, store {}

public struct BalanceReserve<phantom T> has store {
    balance: Balance<T>,
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

public fun authenticate(
    account: &SpendLimit,
    signature: vector<u8>,
    auth_ctx: &AuthContext,
    ctx: &TxContext,
) {
    iotaccount::ensure_tx_sender_is_account_id(&account.id, ctx);

    owner_public_key::authenticate_ed25519(&account.id, signature, ctx.digest());

    assert!(has_right_package_id_and_withdraw_call(auth_ctx, ctx), EUnauthorizedWithdrawCall);

    let actual_amount = extract_withdraw_amount(auth_ctx);

    spending_limit::authenticate_with_amount(&account.id, actual_amount);
}

public fun withdraw_from_balance_reserve(
    self: &mut iotaccount::IOTAccount,
    amount: u64,
    ctx: &mut TxContext,
): Coin<iota::iota::IOTA> {
    iotaccount::ensure_tx_sender_is_account(self, ctx);

    let reserve: &mut BalanceReserve<iota::iota::IOTA> = self.borrow_field_mut(
        BalanceReserveKey {},
        ctx,
    );

    assert!(balance::value(&reserve.balance) >= amount, EInsufficientBalanceReserve);

    let withdrawn_balance = balance::split(&mut reserve.balance, amount);
    coin::from_balance(withdrawn_balance, ctx)
}

fun extract_withdraw_amount(auth_ctx: &AuthContext): u64 {
    let commands = tx_commands(auth_ctx);
    let inputs = tx_inputs(auth_ctx);

    // Find the withdraw_from_balance_reserve call
    let withdraw_idx = commands.find_index!(|cmd| {
        if (command_to_int(cmd) != 0) {
            false
        } else {
            let call = move_call_data(cmd);
            function_name(call) == &ascii::string(b"withdraw_from_balance_reserve")
        }
    });

    // Ensure we found the withdraw call
    assert!(option::is_some(&withdraw_idx), EUnauthorizedWithdrawCall);

    // Get the command at that index
    let cmd_index = option::destroy_some(withdraw_idx);
    let cmd = commands.borrow(cmd_index);
    let call = move_call_data(cmd);

    // Extract the amount argument (second arg after self)
    let args = arguments(call);
    assert!(args.length() >= 2, EInvalidArgumentType);

    let amount_arg = args.borrow(1);
    let input_ix = argument_input(amount_arg) as u64;

    assert!(input_ix < inputs.length(), EInvalidArgumentType);

    let call_arg = inputs.borrow(input_ix);
    assert!(is_pure_data(call_arg), EInvalidArgumentType);

    let pure_bytes = call_arg.pure_data();
    let mut bcs_reader = bcs::new(*pure_bytes);
    return bcs::peel_u64(&mut bcs_reader)
}

public fun has_right_package_id_and_withdraw_call(
    auth_ctx: &AuthContext,
    ctx: &iota::tx_context::TxContext,
): bool {
    let commands = tx_commands(auth_ctx);
    let hit = commands.find_index!(|cmd| {
        if (command_to_int(cmd) != 0) {
            return false
        };

        let call = move_call_data(cmd);

        // Check first argument equals sender
        if (!first_arg_equals_sender(call, auth_ctx, ctx)) {
            return false
        };

        // Check if the function is withdraw_from_balance_reserve
        if (function_name(call) != &ascii::string(b"withdraw_from_balance_reserve")) {
            return false
        };

        if (module_name(call) != &ascii::string(b"account")) {
            return false
        };

        // Extract the package ID from the call (convert ID -> address)
        let call_package_id = package_id(call);
        let call_package_addr = object::id_to_address(call_package_id);

        let expected_type = get<SpendLimit>();
        let expected_addr_string = get_address(&expected_type);

        // Convert the ASCII string to an address for comparison
        let expected_package_addr = iota::address::from_ascii_bytes(expected_addr_string.as_bytes());

        // Compare the two addresses
        call_package_addr == expected_package_addr
    });
    option::is_some(&hit)
}

fun first_arg_equals_sender(
    call: &ProgrammableMoveCall,
    auth_ctx: &AuthContext,
    ctx: &tx_context::TxContext,
): bool {
    // Read the MoveCall's argument list and get arg0
    let args = arguments(call);
    if (args.length() == 0) {
        return false
    };

    let arg0 = args.borrow(0);

    // u64 since then borrow and length are u64 as well
    let input_ix = argument_input(arg0) as u64;

    let inputs = tx_inputs(auth_ctx);

    if (input_ix >= (inputs.length())) {
        return false
    };
    let carg = inputs.borrow(input_ix);

    // I guess it's not expected to have like a pure input...
    if (is_pure_data(carg)) {
        return false
    };

    // Object argument where its ID/address equals sender
    if (is_object_data(carg)) {
        let obj_data = carg.object_data();

        // Need to check if it's a shared object

        if (is_shared_object(obj_data)) {
            let (shared_id, _, _) = shared_object_data(obj_data);
            let id_addr = object::id_to_address(&shared_id);
            return id_addr == tx_context::sender(ctx)
        };

        // It's either an owned or immutable object then

        let obj_id = obj_data.object_ref().object_id();
        let id_addr = object::id_to_address(obj_id);
        return id_addr == tx_context::sender(ctx)
    };

    false
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

// === Admin Functions ===

// === Package Functions ===

// === Private Functions ===

// === Test Functions ===
