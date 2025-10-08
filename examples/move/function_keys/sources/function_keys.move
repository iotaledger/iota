// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Public API & authenticator for per-account Function Keys (allow-set).
///
/// This module provides:
/// - `create` to initialize the per-account allow-set (a dynamic field).
/// - `grant_permission` / `revoke_permission` admin operations.
/// - `has_permission` read-only query.
/// - `authenticate` implementation that:
///     1. delegates signature verification to `iotaccount::authenticate_ed25519`
///        (so it uses the **account’s** single stored public key),
///     2. requires **exactly one** PTB command,
///     3. extracts the called `FunctionKey` and checks membership in the allow-set.
///
module function_keys::function_keys;

use function_keys::fk_store::{
    extract_func_key,
    FunctionKey,
    attach_fk_store,
    borrow_fk_store,
    borrow_fk_store_mut,
    fk_store_exists,
    allow,
    disallow,
    is_allowed
};
use iota::auth_context::AuthContext;
use iota::ed25519;
use iota::hex::decode;
use iotaccount::iotaccount::IOTAccount;

// --------------------
// Errors
// --------------------

/// DF missing (forgot to `create`).
#[error(code = 0)]
const EFunctionKeysNotInitialized: vector<u8> = b"The function key has not been initializaed";
/// PTB does not contain **exactly one** command.
#[error(code = 1)]
const EInvalidAmountOfCommands: vector<u8> = b"Invalid number of commands";
/// Called function not in the allow-set.
#[error(code = 2)]
const EUnauthorized: vector<u8> = b"Function key is not the allowed set";
/// Ed225519 verification has failed.
#[error(code = 3)]
const EEd25519VerificationFailed: vector<u8> = b"Ed25519 verification has failed";


/// Initializes the Function Keys store under the given `account`.
public fun create(account: &mut IOTAccount, ctx: &mut TxContext) {
    attach_fk_store(account, ctx);
}

/// Grants (allows) a `FunctionKey` under a specific `pub_key`.
/// - Only the account owner may mutate their DF.
public fun grant_permission(
    account: &mut IOTAccount,
    pub_key: vector<u8>,
    func_key: FunctionKey,
    ctx: &mut TxContext,
) {
    account.ensure_tx_sender_is_account(ctx);
    assert!(fk_store_exists(account), EFunctionKeysNotInitialized);

    let fk_store = borrow_fk_store_mut(account, ctx);
    fk_store.allow(pub_key, func_key);
}

/// Revokes (disallows) a `FunctionKey` under a specific `pub_key`.
public fun revoke_permission(
    account: &mut IOTAccount,
    pub_key: vector<u8>,
    func_key: &FunctionKey,
    ctx: &TxContext,
) {
    account.ensure_tx_sender_is_account(ctx);
    assert!(fk_store_exists(account), EFunctionKeysNotInitialized);

    let fk_store = borrow_fk_store_mut(account, ctx);
    fk_store.disallow(pub_key, func_key);
}

/// Read-only query for membership in the per-pubkey allow-set.
public fun has_permission(account: &IOTAccount, pub_key: vector<u8>, func_key: &FunctionKey): bool {
    if (!fk_store_exists(account)) return false;
    let fk_store = borrow_fk_store(account);
    fk_store.is_allowed(pub_key, func_key)
}
// --------------------
// Authenticator
// --------------------

/// Authenticates a transaction using the **account’s** ed25519 public key
/// and the per-account **allow-set** of function keys.
///
/// Steps:
/// 1. `iotaccount::authenticate_ed25519(account, signature, auth_ctx, ctx)` verifies the signature
///    against the account’s stored public key and the canonical `ctx.digest()`.
/// 2. Check `auth_ctx.tx_commands().length() == 1` to enforce a single operation PTB.
/// 3. Convert the sole `Command` into a `FunctionKey` via `extract_func_key(...)`.
/// 4. Assert the key exists in the account’s allow-set.
///
/// Errors:
/// - `EFunctionKeysNotInitialized` if `create` has not been called.
/// - `EInvalidAmountOfCommands` if PTB has 0 or > 1 commands.
/// - `EUnauthorized` if the call target isn’t allowed.
public fun authenticate(
    account: &IOTAccount,
    pub_key: vector<u8>,
    signature: vector<u8>,
    auth_ctx: &AuthContext,
    ctx: &TxContext,
) {
    assert!(fk_store_exists(account), EFunctionKeysNotInitialized);

    // Check the signature.
    assert!(
        ed25519::ed25519_verify(&decode(signature), &pub_key, ctx.digest()),
        EEd25519VerificationFailed,
    );

    // PTB MUST contain exactly one command.
    assert!(auth_ctx.tx_commands().length() == 1, EInvalidAmountOfCommands);

    let func_key = extract_func_key(&auth_ctx.tx_commands()[0]);
    let fk_store = borrow_fk_store(account);
    assert!(fk_store.is_allowed(pub_key, &func_key), EUnauthorized);
}
