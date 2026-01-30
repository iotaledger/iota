// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module spending_limit::account;

use generic_keyed_authentication::owner_public_key;
use iota::account;
use iota::auth_context::{tx_commands, tx_inputs};
use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::balance::{Self, Balance};
use iota::bcs;
use iota::coin::{Self, Coin};
use iota::dynamic_field;
use iota::iota::IOTA;
use iota::ptb_call_arg::{
    is_object_data,
    is_pure_data,
    is_shared_object,
    as_pure_data,
    as_object_data,
    object_id
};
use iota::ptb_command::{
    module_name,
    function as function_name,
    package as package_id,
    arguments,
    as_move_call,
    ProgrammableMoveCall,
    input_index
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

#[error(code = 3)]
const ESpendingLimitExceeded: vector<u8> = b"Amount exceeds spending limit.";

// === Constants ===

// === Structs ===

/// Struct for the SpendLimit account.
public struct SpendLimit has key {
    id: UID,
}

/// Marker for the gas reserve balance (outside spending limit).
public struct BalanceReserveKey has copy, drop, store {}

/// Struct for the balance reserve to keep in the account.
public struct BalanceReserve has store {
    balance: Balance<IOTA>,
}

// === Events ===

// === Method Aliases ===

// === Public Functions ===

/// Create a new SpendLimit account. Initializes the account with the given public key and spending limit.
public fun create(
    public_key: vector<u8>,
    limit: u64,
    authenticator: AuthenticatorFunctionRefV1<SpendLimit>,
    ctx: &mut TxContext,
) {
    // Create the SpendLimit account object.
    let mut spend_limit_account = SpendLimit { id: object::new(ctx) };

    // Attach public key using the owner_public_key module.
    owner_public_key::attach(&mut spend_limit_account.id, public_key);

    // Initialize balance reserve.
    dynamic_field::add(
        &mut spend_limit_account.id,
        BalanceReserveKey {},
        BalanceReserve {
            balance: balance::zero<IOTA>(),
        },
    );

    // Attach spending limit.
    spending_limit::attach(
        &mut spend_limit_account.id,
        limit,
    );

    // Finalize account creation.
    account::create_account_v1(spend_limit_account, authenticator);
}

/// Withdraws the specified amount from the balance reserve of the SpendLimit account.
/// Ensures that the transaction sender is the account itself.
public fun withdraw_from_balance_reserve(
    self: &mut SpendLimit,
    amount: u64,
    ctx: &mut TxContext,
): Coin<IOTA> {
    // Consume and validate proof.
    let reserve: &mut BalanceReserve = borrow_field_mut(
        self,
        BalanceReserveKey {},
        ctx,
    );

    assert!(balance::value(&reserve.balance) >= amount, EInsufficientBalanceReserve);
    let withdrawn_balance = balance::split(&mut reserve.balance, amount);
    let spending_limit: &mut u64 = spending_limit::borrow_mut(&mut self.id);
    assert!(*spending_limit >= amount, ESpendingLimitExceeded);
    *spending_limit = *spending_limit - amount;

    coin::from_balance(withdrawn_balance, ctx)
}

/// Deposit coins into the balance reserve of the SpendLimit account.
public fun deposit_to_reserve(self: &mut SpendLimit, coin: Coin<IOTA>) {
    let reserve = dynamic_field::borrow_mut<BalanceReserveKey, BalanceReserve>(
        &mut self.id,
        BalanceReserveKey {},
    );
    balance::join(&mut reserve.balance, coin::into_balance(coin));
}

/// Borrow a mutable dynamic field from the SpendLimit account.
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

/// Ensure that the transaction sender is the SpendLimit account itself.
public fun ensure_tx_sender_is_account(self: &SpendLimit, ctx: &TxContext) {
    assert!(self.id.uid_to_address() == ctx.sender(), ETransactionSenderIsNotTheAccount);
}

// === Authenticators ===

/// Authenticator function for SpendLimit accounts.
/// Validates the signature and the withdrawal commands in the transaction.
/// Calculates the total withdrawal amount and checks against the spending limit.
#[authenticator]
public fun authenticate(
    account: &SpendLimit,
    signature: vector<u8>,
    auth_ctx: &AuthContext,
    ctx: &TxContext,
) {
    owner_public_key::authenticate_ed25519(&account.id, signature, ctx.digest());

    let total_amount = lookup_and_calculate_withdrawals(auth_ctx, ctx);

    spending_limit::check_amount_against_spending_limit(&account.id, total_amount);
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

// Get the authenticator function ref.
public fun authenticator_function_ref(
    account: &SpendLimit,
): &AuthenticatorFunctionRefV1<SpendLimit> {
    account::borrow_auth_function_ref_v1<SpendLimit>(&account.id)
}

// === Admin Functions ===

// === Package Functions ===

/// Looks up for withdraw calls and calculates total withdrawal amount.
/// Returns the total amount from all withdraw commands.
/// Returns 0 if no withdraw commands are found.
public(package) fun lookup_and_calculate_withdrawals(auth_ctx: &AuthContext, ctx: &TxContext): u64 {
    let commands = tx_commands(auth_ctx);
    let inputs = tx_inputs(auth_ctx);
    let mut total_amount = 0u64;
    let mut i = 0;
    let len = commands.length();

    while (i < len) {
        let cmd = &commands[i];

        let call_opt = as_move_call(cmd);

        if (option::is_some(&call_opt)) {
            let call = option::borrow(&call_opt);

            if (is_withdraw_call(call, auth_ctx, ctx)) {
                // Extract amount inline.
                let args = arguments(call);
                assert!(args.length() > 1, EInvalidAmount);
                let amount_arg = &args[1];
                let input_idx_opt = input_index(amount_arg);
                assert!(option::is_some(&input_idx_opt), EInvalidAmount);
                let input_idx = *option::borrow(&input_idx_opt);
                assert!((input_idx as u64) < inputs.length(), EInvalidAmount);
                let call_arg = &inputs[(input_idx as u64)];
                let bytes_opt = as_pure_data(call_arg);
                assert!(option::is_some(&bytes_opt), EInvalidAmount);
                let bytes = *option::borrow(&bytes_opt);
                // u64 is 8 bytes
                assert!(bytes.length() == 8, EInvalidAmount);
                let mut bcs_stream = bcs::new(bytes);
                let amount = bcs_stream.peel_u64();
                assert!(amount > 0, EInvalidAmount);

                total_amount = total_amount + amount;
            }
        };

        i = i + 1;
    };

    total_amount
}

// === Private Functions ===

// Helper function to check if a MoveCall is a withdraw_from_balance_reserve call from the account module.
fun is_withdraw_call(call: &ProgrammableMoveCall, auth_ctx: &AuthContext, ctx: &TxContext): bool {
    // Check first argument equals sender.
    if (!first_arg_equals_sender(call, auth_ctx, ctx)) {
        return false
    };

    // Check if the function is withdraw_from_balance_reserve.
    if (function_name(call) != &ascii::string(b"withdraw_from_balance_reserve")) {
        return false
    };

    // Check if the module is account.
    if (module_name(call) != &ascii::string(b"account")) {
        return false
    };

    // Extract the package ID from the call (convert ID -> address).
    let call_package_id = package_id(call);
    let call_package_addr = object::id_to_address(call_package_id);

    let expected_type = get<SpendLimit>();
    let expected_addr_string = get_address(&expected_type);

    // Convert the ASCII string to an address for comparison.
    let expected_package_addr = iota::address::from_ascii_bytes(expected_addr_string.as_bytes());

    // Compare the two addresses.
    call_package_addr == expected_package_addr
}

// Helper function to check if the first argument of the MoveCall equals the transaction sender.
fun first_arg_equals_sender(
    call: &ProgrammableMoveCall,
    auth_ctx: &AuthContext,
    ctx: &tx_context::TxContext,
): bool {
    // Read the MoveCall's argument list and get arg0.
    let args = arguments(call);
    if (args.is_empty()) {
        return false
    };

    let arg0 = args.borrow(0);

    // u64 since then borrow and length are u64 as well.
    let input_ix_opt = input_index(arg0);
    assert!(option::is_some(&input_ix_opt), EInvalidAmount);
    let input_ix = *option::borrow(&input_ix_opt) as u64;

    let inputs = tx_inputs(auth_ctx);

    if (input_ix >= (inputs.length())) {
        return false
    };
    let carg = inputs.borrow(input_ix);

    // Pure data argument cannot be equal to sender.
    if (is_pure_data(carg)) {
        return false
    };

    // Object argument where its ID/address equals sender.
    if (is_object_data(carg)) {
        let obj_data_opt = carg.as_object_data();

        assert!(option::is_some(&obj_data_opt), EInvalidAmount);
        let obj_data = option::borrow(&obj_data_opt);
        // Need to check if it's a shared object.

        if (is_shared_object(obj_data)) {
            let shared_id_opt = object_id(obj_data);
            assert!(option::is_some(&shared_id_opt), EInvalidAmount);
            let shared_id = *option::borrow(&shared_id_opt);
            let id_addr = object::id_to_address(&shared_id);
            return id_addr == tx_context::sender(ctx)
        };

        // It's either an owned or immutable object then.

        let obj_id_opt = object_id(obj_data);
        assert!(option::is_some(&obj_id_opt), EInvalidAmount);
        let obj_id = *option::borrow(&obj_id_opt);
        let id_addr = object::id_to_address(&obj_id);
        return id_addr == tx_context::sender(ctx)
    };

    false
}

// === Test Functions ===

// Useless function to test withdrawals in programmable transactions calling this function instead of withdraw_from_balance_reserve.
#[test_only]
public fun random_function_that_does_nothing(_number: u16) {}

// Helper test function to retrieve the BalanceReserveKey.
#[test_only]
public fun get_balance_reserve_key_for_testing(): BalanceReserveKey {
    BalanceReserveKey {}
}
