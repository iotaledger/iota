// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module spending_limit::account;

use generic_keyed_authentication::owner_public_key;
use iota::account::{Self, AuthenticatorInfoV1};
use iota::auth_context::{AuthContext, tx_commands, tx_inputs};
use iota::balance::{Self, Balance};
use iota::bcs;
use iota::coin::{Self, Coin};
use iota::dynamic_field;
use iota::iota::IOTA;
use iota::programmable_transaction::{
    move_call_data,
    command_to_int,
    pure_data,
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
use spending_limit::spending_limit;
use std::ascii;
use std::type_name::{get, get_address};

// === Errors ===

#[error(code = 0)]
const EInsufficientBalanceReserve: vector<u8> = b"Insufficient balance reserve.";

#[error(code = 1)]
const ETransactionSenderIsNotTheAccount: vector<u8> = b"Transaction must be signed by the account.";

#[error(code = 2)]
const EInvalidAmount: vector<u8> = b"Invalid amount in withdraw command.";

// === Constants ===

// === Structs ===

public struct SpendLimit has key {
    id: UID,
}

// Marker for the gas reserve balance (outside spending limit).
public struct BalanceReserveKey has copy, drop, store {}

public struct BalanceReserve has store {
    balance: Balance<IOTA>,
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
    // Initialize balance reserve.
    dynamic_field::add(
        &mut id,
        BalanceReserveKey {},
        BalanceReserve {
            balance: balance::zero<IOTA>(),
        },
    );
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
): u64 {
    owner_public_key::authenticate_ed25519(&account.id, signature, ctx.digest());

    let total_amount = validate_and_calculate_withdrawals(auth_ctx, ctx);

    spending_limit::authenticate_with_amount(&account.id, total_amount);

    total_amount
}

public fun withdraw_from_balance_reserve(
    self: &mut SpendLimit,
    amount: u64,
    ctx: &mut TxContext,
): Coin<IOTA> {
    // Consume and validate proof
    let reserve: &mut BalanceReserve = borrow_field_mut(
        self,
        BalanceReserveKey {},
        ctx,
    );

    assert!(balance::value(&reserve.balance) >= amount, EInsufficientBalanceReserve);
    let withdrawn_balance = balance::split(&mut reserve.balance, amount);
    coin::from_balance(withdrawn_balance, ctx)
}

// Validates withdraw calls and calculates total withdrawal amount.
// Returns the total amount from all valid withdraw commands.
// Returns 0 if no valid withdraw commands are found.
public(package) fun validate_and_calculate_withdrawals(
    auth_ctx: &AuthContext,
    ctx: &TxContext,
): u64 {
    let commands = tx_commands(auth_ctx);
    let inputs = tx_inputs(auth_ctx);
    let mut total_amount = 0u64;
    let mut i = 0;
    let len = commands.length();

    while (i < len) {
        let cmd = &commands[i];

        if (command_to_int(cmd) == 0) {
            let call = move_call_data(cmd);

            if (is_valid_withdraw_call(call, auth_ctx, ctx)) {
                // Extract amount inline
                let args = arguments(call);
                assert!(args.length() > 1, EInvalidAmount);
                let amount_arg = &args[1];
                let input_idx = argument_input(amount_arg);
                assert!((input_idx as u64) < inputs.length(), EInvalidAmount);
                let call_arg = &inputs[(input_idx as u64)];
                let bytes = pure_data(call_arg);
                // u64 is 8 bytes
                assert!(bytes.length() == 8, EInvalidAmount);
                let mut bcs_stream = bcs::new(*bytes);
                let amount = bcs_stream.peel_u64();
                assert!(amount > 0, EInvalidAmount);

                total_amount = total_amount + amount;
            };
        };

        i = i + 1;
    };

    total_amount
}

// Helper function to validate if a MoveCall is a valid withdraw call
fun is_valid_withdraw_call(
    call: &ProgrammableMoveCall,
    auth_ctx: &AuthContext,
    ctx: &TxContext,
): bool {
    // Check first argument equals sender
    if (!first_arg_equals_sender(call, auth_ctx, ctx)) {
        return false
    };

    // Check if the function is withdraw_from_balance_reserve
    if (function_name(call) != &ascii::string(b"withdraw_from_balance_reserve")) {
        return false
    };

    // Check if the module is account
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

    // Pure data argument cannot be equal to sender
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

public fun ensure_tx_sender_is_account(self: &SpendLimit, ctx: &TxContext) {
    assert!(self.id.uid_to_address() == ctx.sender(), ETransactionSenderIsNotTheAccount);
}

public fun deposit_to_reserve(self: &mut SpendLimit, coin: Coin<IOTA>, ctx: &TxContext) {
    let reserve = borrow_field_mut<BalanceReserveKey, BalanceReserve>(
        self,
        BalanceReserveKey {},
        ctx,
    );
    balance::join(&mut reserve.balance, coin::into_balance(coin));
}

public fun borrow_field_mut<Name: copy + drop + store, Value: store>(
    self: &mut SpendLimit,
    name: Name,
    ctx: &TxContext,
): &mut Value {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    // Borrow the related dynamic field.
    dynamic_field::borrow_mut(&mut self.id, name)
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

// Useless function to test withdrawals in programmable transactions calling this function instead of withdraw_from_balance_reserve.
#[test_only]
public fun random_function_that_does_nothing(_number: u16) {}
