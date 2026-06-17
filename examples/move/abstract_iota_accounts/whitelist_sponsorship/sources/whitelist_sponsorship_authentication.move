// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// This module owns the authenticator surface of `WhitelistSponsorshipAccount`. It is split out
/// from `whitelist_sponsorship_account` so the storage/admin module stays focused on state
/// management and so the authenticator's PTB-scan helpers (which would otherwise be private
/// helpers inside the storage module) live alongside the `#[authenticator]` function they
/// support.
///
/// The authenticator reads the account's policy state through the public views exposed by
/// `whitelist_sponsorship_account` — it never reaches into the struct's fields directly.
module whitelist_sponsorship::whitelist_sponsorship_authentication;

use iota::auth_context::AuthenticatorFunctionInfoV1;
use iota::ptb_command::ProgrammableMoveCall;
use std::ascii;
use whitelist_sponsorship::whitelist_sponsorship_account::{
    Self,
    AuthenticatorFunctionKey,
    WhitelistSponsorshipAccount
};

/// Method-syntax alias for `ptb_command::function`, which clashes with the `function_name`
/// accessor on `AuthenticatorFunctionKey`.
use fun iota::ptb_command::function as ProgrammableMoveCall.move_call_function;

// === Errors ===

#[error(code = 0)]
const ENotASponsoredTransaction: vector<u8> = b"Transaction is not sponsored by this account.";

#[error(code = 1)]
const ESenderAuthenticatorFunctionMissing: vector<u8> = b"Sender does not use a MoveAuthenticator.";

#[error(code = 2)]
const EAuthenticatorFunctionNotWhitelisted: vector<u8> = b"Authenticator function not whitelisted.";

#[error(code = 3)]
const EGasBudgetExceedsAllowance: vector<u8> =
    b"Transaction gas budget exceeds the sponsored user's allowance.";

#[error(code = 4)]
const EDeductCallMissing: vector<u8> =
    b"PTB does not contain a `deduct_user_gas_allowance` call for this sponsor and sender.";

// === Constants ===

/// The module name of `whitelist_sponsorship_account`, used by the PTB scan to identify calls
/// to `deduct_user_gas_allowance`.
const DEDUCT_USER_GAS_ALLOWANCE_MODULE_NAME: vector<u8> = b"whitelist_sponsorship_account";

/// The function name of `deduct_user_gas_allowance` in `whitelist_sponsorship_account`, used by
/// the PTB scan.
const DEDUCT_USER_GAS_ALLOWANCE_FUNC_NAME: vector<u8> = b"deduct_user_gas_allowance";

// === Authenticators ===

/// Authenticator for `WhitelistSponsorshipAccount`.
///
/// Aborts if:
/// - the transaction is not sponsored by this account,
/// - the sender does not use a `MoveAuthenticator`,
/// - the sender's authenticator function is not in the whitelist,
/// - the sender has no gas allowance,
/// - the transaction gas budget exceeds the sender's allowance,
/// - the PTB does not include a `deduct_user_gas_allowance` call for this sponsor.
#[authenticator]
public fun authenticator(
    account: &WhitelistSponsorshipAccount,
    auth_ctx: &AuthContext,
    ctx: &TxContext,
) {
    // Check if the transaction is sponsored by this account.
    let sponsor_address = account.account_address();
    assert!(ctx.sponsor() == option::some(sponsor_address), ENotASponsoredTransaction);

    // Check that the sender uses a `MoveAuthenticator` whose function is in the whitelist.
    let sender_info_opt = auth_ctx.sender_authenticator_function_info_v1();
    assert!(sender_info_opt.is_some(), ESenderAuthenticatorFunctionMissing);
    let key = key_from_info(sender_info_opt.borrow());
    assert!(
        account.is_authenticator_function_whitelisted(key),
        EAuthenticatorFunctionNotWhitelisted,
    );

    // Check that the transaction gas budget fits within the sender's allowance.
    // `borrow` itself aborts (with `iota::dynamic_field::EFieldDoesNotExist`) when the sender
    // has no entry, so we skip an explicit `contains` check to save one `df::exists_`.
    let allowances = account.borrow_user_gas_allowances();
    assert!(
        ctx.gas_budget() <= *allowances.borrow(ctx.sender()),
        EGasBudgetExceedsAllowance,
    );

    // Finally, confirm the sender included a `deduct_user_gas_allowance` call in the PTB.
    // The call implicitly targets `ctx.sender()` and always deducts exactly `ctx.gas_budget()`,
    // so the authenticator only needs to confirm such a call exists for this sponsor — no user
    // or amount argument is read here. The expected package address is read from the account's
    // cached `package_addr` field, set once at `create` time via `type_name::get`, so the hot
    // path doesn't pay for runtime reflection.
    assert!(
        has_matching_deduct_call(sponsor_address, account.borrow_package_addr(), auth_ctx),
        EDeductCallMissing,
    );
}

// === Private Functions ===

/// Derives an `AuthenticatorFunctionKey` from the framework's type-erased
/// `AuthenticatorFunctionInfoV1` returned by `AuthContext`.
fun key_from_info(info: &AuthenticatorFunctionInfoV1): AuthenticatorFunctionKey {
    whitelist_sponsorship_account::new_authenticator_function_key(
        info.package(),
        *info.module_name(),
        *info.function_name(),
    )
}

/// Scans the PTB commands for a call to `deduct_user_gas_allowance` on this sponsor account,
/// returning `true` on the first match.
fun has_matching_deduct_call(
    sponsor_address: address,
    expected_package_addr: address,
    auth_ctx: &AuthContext,
): bool {
    let commands = auth_ctx.tx_commands();
    let inputs = auth_ctx.tx_inputs();

    let expected_module = ascii::string(DEDUCT_USER_GAS_ALLOWANCE_MODULE_NAME);
    let expected_function = ascii::string(DEDUCT_USER_GAS_ALLOWANCE_FUNC_NAME);

    'found: {
        commands.do_ref!(|command| {
            command.as_move_call().do!(|call| {
                if (call.move_call_function() != &expected_function) return;
                if (call.module_name() != &expected_module) return;
                if (object::id_to_address(call.package()) != expected_package_addr) return;

                // Args: [sponsor_account]. The sponsor account argument must be an object input
                // (not a pure input) whose ID equals the sponsor address.
                let args = call.arguments();
                let input_ix = args[0].input_index().destroy_some() as u64;
                let call_arg = &inputs[input_ix];
                if (call_arg.is_pure_data()) return;
                let obj_data = call_arg.as_object_data().destroy_some();
                let obj_id = obj_data.object_id().destroy_some();
                if (object::id_to_address(&obj_id) != sponsor_address) return;

                return 'found true;
            });
        });
        false
    }
}
