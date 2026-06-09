// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// The SpendingLimitAccount module defines an account struct that can be used as a programmable account
/// with a spending limit.
///
/// The account data, stored as dynamic fields, includes a spending limit value and a balance reserve.
/// The spending limit is a u64 value that represents the maximum amount that can be withdrawn from the
/// account in a single transaction. The balance reserve is a struct that holds the current balance
/// reserved for spending and allows withdrawing and depositing funds to it. The account also has an
/// owner public key.
///
/// The module includes functions to create a new `SpendingLimitAccount`, rotate the account's
/// authenticator, rotate the account's owner public key, withdraw from the balance reserve, and deposit
/// to the balance reserve. It also includes view functions to query the account's UID, address,
/// spending limit, owner public key, and authenticator function reference.
///
/// The authenticator function for the `SpendingLimitAccount` validates the signature and checks for
/// withdrawal commands in the transaction PTB. It looks into the PTB commands to find calls to the
/// `withdraw_from_balance_reserve` function, calculates the total amount to be withdrawn in the
/// transaction, and checks that the total amount does not exceed the spending limit.
module spending_limit::spending_limit_account;

use iota::account;
use iota::auth_context::{tx_commands, tx_inputs};
use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::bcs;
use iota::coin::Coin;
use iota::iota::IOTA;
use iota::ptb_call_arg::{is_pure_data, as_pure_data, as_object_data, object_id};
use iota::ptb_command::{
    module_name,
    function as function_name,
    package as package_id,
    arguments,
    as_move_call,
    ProgrammableMoveCall,
    input_index
};
use public_key_authentication::public_key_authentication;
use spending_limit::balance_reserve;
use spending_limit::spending_limit_authentication;
use std::ascii;
use std::type_name;

/// Allows calling `.borrow_mut_balance_reserve` on an `UID` to borrow a `BalanceReserve`.
use fun balance_reserve::borrow_mut_balance_reserve as UID.borrow_mut_balance_reserve;

/// Allows calling `.borrow_spending_limit` on an `UID` to borrow a `SpendingLimit`.
use fun spending_limit_authentication::borrow_spending_limit as UID.borrow_spending_limit;

/// Allows calling `.rotate_spending_limit` on an `UID` to borrow a `SpendingLimit`.
use fun spending_limit_authentication::rotate_spending_limit as UID.rotate_spending_limit;

/// Allows calling `.is_withdraw_call` on a `ProgrammableMoveCall` directly.
use fun is_withdraw_call as ProgrammableMoveCall.is_withdraw_call;

/// Allows calling `.first_arg_equals_sender` on a `ProgrammableMoveCall` directly.
use fun first_arg_equals_sender as ProgrammableMoveCall.first_arg_equals_sender;

// === Errors ===

#[error(code = 0)]
const ETransactionSenderIsNotTheAccount: vector<u8> = b"Transaction must be signed by the account.";
#[error(code = 1)]
const EInvalidAmount: vector<u8> = b"Invalid amount in withdraw command.";
#[error(code = 2)]
const ESpendingLimitExceeded: vector<u8> = b"Amount exceeds spending limit.";

// === Constants ===

/// The name of the `withdraw_from_balance_reserve` function, used for looking up calls to that function
/// in the transaction commands.
const WITHDRAW_FROM_BALANCE_RESERVE_FUNC_NAME: vector<u8> = b"withdraw_from_balance_reserve";

/// The name of the `account` module, used for looking up calls to `withdraw_from_balance_reserve` in the
/// transaction commands.
const ACCOUNT_MODULE_NAME: vector<u8> = b"account";

// === Structs ===

/// Struct for the SpendingLimitAccount account.
public struct SpendingLimitAccount has key {
    id: UID,
}

// === SpendingLimitAccount Handling ===

/// Create a new `SpendingLimitAccount` as a shared object with the given authenticator.
///
/// Initializes the account with some given public key and spending limit.
public fun create(
    public_key: vector<u8>,
    limit: u64,
    authenticator: AuthenticatorFunctionRefV1<SpendingLimitAccount>,
    ctx: &mut TxContext,
) {
    // Create the SpendingLimitAccount account object.
    let mut spend_limit_account = SpendingLimitAccount { id: object::new(ctx) };
    let id = &mut spend_limit_account.id;

    // Attach public key using the public_key_authentication module.
    public_key_authentication::attach_public_key(id, public_key);

    // Initialize balance reserve.
    balance_reserve::attach_balance_reserve(id, balance_reserve::new_empty_balance_reserve<IOTA>());

    // Attach spending limit.
    spending_limit_authentication::attach_spending_limit(
        id,
        limit,
    );

    // Finalize account creation.
    account::create_account_v1(spend_limit_account, authenticator);
}

/// Rotate the attached authenticator.
///
/// Only the account itself or the admin can call this function.
public fun rotate_auth_function_ref_v1(
    self: &mut SpendingLimitAccount,
    authenticator: AuthenticatorFunctionRefV1<SpendingLimitAccount>,
    ctx: &TxContext,
): AuthenticatorFunctionRefV1<SpendingLimitAccount> {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    account::rotate_auth_function_ref_v1(self, authenticator)
}

/// Rotates the account owner public key to a new one as well as the authenticator.
/// Once this function is called, the previous public key and authenticator are no longer valid.
/// Only the account itself can call this function.
public fun rotate_public_key(
    account: &mut SpendingLimitAccount,
    public_key: vector<u8>,
    authenticator: AuthenticatorFunctionRefV1<SpendingLimitAccount>,
    ctx: &TxContext,
) {
    // Update the account owner public key dynamic field. It is expected that the field already exists.
    public_key_authentication::rotate_public_key(&mut account.id, public_key);
    // Update the account authenticator dynamic field. It is expected that the field already exists.
    account.rotate_auth_function_ref_v1(authenticator, ctx);
}

// === Authenticators ===

/// Authenticator function for SpendingLimitAccount accounts.
/// Validates the signature and the withdrawal commands in the transaction.
/// Calculates the total withdrawal amount and checks against the spending limit.
#[authenticator]
public fun ed25519_authenticator(
    account: &SpendingLimitAccount,
    signature: vector<u8>,
    auth_ctx: &AuthContext,
    ctx: &TxContext,
) {
    let account_id = account.borrow_uid();

    // Check signature first.
    public_key_authentication::authenticate_ed25519(account_id, signature, ctx);

    // Then check the presence of withdrawal commands and calculate the total amount to withdraw.
    let total_amount = lookup_and_calculate_withdrawals(auth_ctx, ctx);

    // Finally authenticate against the spending limit.
    spending_limit_authentication::authenticate_spending_limit(
        account_id,
        total_amount,
    );
}

// === SpendingLimitAccount Modification Functions ===

/// Withdraws the specified amount from the balance reserve of the SpendingLimitAccount account.
/// Ensures that the transaction sender is the account itself.
public fun withdraw_from_balance_reserve(
    self: &mut SpendingLimitAccount,
    amount: u64,
    ctx: &mut TxContext,
): Coin<IOTA> {
    // Check that the amount to withdraw is within the spending limit.
    let spending_limit = *self.id.borrow_spending_limit();
    assert!(amount <= spending_limit, ESpendingLimitExceeded);

    // Check if the balance reserve has enough funds and withdraw the amount from that.
    let coin = self.id.borrow_mut_balance_reserve().withdraw_from_balance_reserve(amount, ctx);

    // Update the spending limit by subtracting the withdrawn amount.
    self.id.rotate_spending_limit(spending_limit - amount);

    coin
}

/// Deposit coins into the balance reserve of the SpendingLimitAccount account.
public fun deposit_to_balance_reserve(self: &mut SpendingLimitAccount, coin: Coin<IOTA>) {
    self.id.borrow_mut_balance_reserve().deposit_to_balance_reserve(coin.into_balance());
}

// === View Functions ===

/// Get the UID of the account.
public fun borrow_uid(self: &SpendingLimitAccount): &UID {
    &self.id
}

/// Query the address of the `SpendingLimitAccount` account.
public fun account_address(self: &SpendingLimitAccount): address {
    self.id.to_address()
}

/// Get the spending limit value.
public fun spending_limit(account: &SpendingLimitAccount): u64 {
    *spending_limit_authentication::borrow_spending_limit(&account.id)
}

/// Get the owner public key.
public fun public_key(account: &SpendingLimitAccount): &vector<u8> {
    public_key_authentication::borrow_public_key(&account.id)
}

/// Get the authenticator function ref.
public fun authenticator_function_ref(
    account: &SpendingLimitAccount,
): &AuthenticatorFunctionRefV1<SpendingLimitAccount> {
    account::borrow_auth_function_ref_v1<SpendingLimitAccount>(&account.id)
}

// === Admin Functions ===

/// Check that the sender of this transaction is the account itself.
fun ensure_tx_sender_is_account(self: &SpendingLimitAccount, ctx: &TxContext) {
    assert!(self.id.uid_to_address() == ctx.sender(), ETransactionSenderIsNotTheAccount);
}

// === Private Functions ===

/// Looks up for withdraw calls and calculates total withdrawal amount.
/// Returns the total amount from all withdraw commands.
/// Returns 0 if no withdraw commands are found.
fun lookup_and_calculate_withdrawals(auth_ctx: &AuthContext, ctx: &TxContext): u64 {
    let commands = auth_ctx.tx_commands();
    let inputs = auth_ctx.tx_inputs();

    let mut total_amount = 0u64;

    // Iterate over the commands and look for calls to the withdraw_from_balance_reserve function.
    commands.do_ref!(|command| {
        command.as_move_call().do!(|call| if (call.is_withdraw_call(auth_ctx, ctx)) {
            // Arguments must be exactly 2: the account and the amount.
            let args = call.arguments();
            assert!(args.length() == 2, EInvalidAmount);

            // The second argument must have an index pointing to a transaction input.
            let amount_arg = &args[1];
            let input_idx = amount_arg.input_index().destroy_some() as u64;
            assert!(input_idx < inputs.length(), EInvalidAmount);

            // The indexed input must be pure data.
            let call_arg = &inputs[input_idx];
            let bytes = call_arg.as_pure_data().destroy_some();

            // The pure data must be a BCS serialized valid u64 amount.
            let mut bcs_stream = bcs::new(bytes);
            let amount = bcs_stream.peel_u64();

            // Accumulate the amount to the total amount to withdraw in this transaction.
            total_amount = total_amount + amount;
        });
    });

    total_amount
}

// Helper function to check if a MoveCall is a withdraw_from_balance_reserve call from the account module.
fun is_withdraw_call(call: &ProgrammableMoveCall, auth_ctx: &AuthContext, ctx: &TxContext): bool {
    if (// Check first argument equals sender.
        !call.first_arg_equals_sender(auth_ctx, ctx)
        // Check if the function is withdraw_from_balance_reserve.
        || call.function_name() != &ascii::string(WITHDRAW_FROM_BALANCE_RESERVE_FUNC_NAME)
        // Check if the module is account.
        || call.module_name() != &ascii::string(ACCOUNT_MODULE_NAME)) {
        return false
    };

    // Extract the package ID from the call (convert ID -> address).
    let call_package_addr = object::id_to_address(call.package_id());

    // Compute the expected package address, derived from the SpendingLimitAccount type address.
    let expected_addr_string = type_name::get_address(&type_name::get<SpendingLimitAccount>());

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
    // Read the MoveCall's argument list and get argument at position 0.
    let args = call.arguments();
    if (args.is_empty()) {
        return false
    };
    let input_ix = args[0].input_index().destroy_some() as u64;

    // Extract the argument value from the transaction inputs.
    let inputs = auth_ctx.tx_inputs();
    if (input_ix >= inputs.length()) {
        return false
    };
    let call_arg = &inputs[input_ix];

    // Pure data argument cannot be equal to sender.
    if (call_arg.is_pure_data()) {
        return false
    };

    // Look for an Object call argument where its ID/address equals sender.
    let obj_data = call_arg.as_object_data().destroy_some();
    let obj_id = object::id_to_address(&obj_data.object_id().destroy_some());

    return obj_id == ctx.sender()
}
