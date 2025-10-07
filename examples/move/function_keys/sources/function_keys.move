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
    fk_store_key,
    new_store,
    borrow_store,
    borrow_store_mut,
    store_exists,
    allow,
    disallow,
    is_allowed
};
use iota::auth_context::AuthContext;
use iotaccount::basic_keyed_account::authenticate_ed25519;
use iotaccount::iotaccount::IOTAccount;

// --------------------
// Errors
// --------------------

/// DF already exists (double init).
const EFunctionKeysAlreadyInitialized: u64 = 1;
/// DF missing (forgot to `create`).
const EFunctionKeysNotInitialized: u64 = 2;
/// PTB does not contain **exactly one** command.
const EInvalidAmountOfCommands: u64 = 3;
/// Called function not in the allow-set.
const EUnauthorized: u64 = 4;

/// Initializes the Function Keys store under the given `account`.
public fun create(account: &mut IOTAccount, ctx: &mut TxContext) {
    assert!(!account.has_field(fk_store_key()), EFunctionKeysAlreadyInitialized);

    let fk_store = new_store();
    account.add_field(fk_store_key(), fk_store, ctx);
}

/// Grants (allows) a `FunctionKey` for this account.
///
/// Behavior:
/// - Aborts if the store is not initialized (`create` not called).
/// - Aborts if `func_key` already present (via `fk_store::allow`).
public fun grant_permission(account: &mut IOTAccount, func_key: FunctionKey, ctx: &mut TxContext) {
    account.ensure_tx_sender_is_account(ctx);
    assert!(store_exists(account), EFunctionKeysNotInitialized);

    let fk_store = borrow_store_mut(account, ctx);
    fk_store.allow(func_key);
}

/// Revokes (disallows) a `FunctionKey` for this account.
///
/// Behavior:
/// - Aborts if missing (via `fk_store::disallow`).
///
public fun revoke_permission(account: &mut IOTAccount, func_key: &FunctionKey, ctx: &TxContext) {
    account.ensure_tx_sender_is_account(ctx);
    assert!(store_exists(account), EFunctionKeysNotInitialized);

    let fk_store = borrow_store_mut(account, ctx);
    fk_store.disallow(func_key);
}

/// Read-only query for membership in the allow-set.
public fun has_permission(account: &IOTAccount, func_key: &FunctionKey): bool {
    if (!store_exists(account)) return false;
    let fk_store = borrow_store(account);
    fk_store.is_allowed(func_key)
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
    signature: vector<u8>,
    auth_ctx: &AuthContext,
    ctx: &TxContext,
) {
    assert!(store_exists(account), EFunctionKeysNotInitialized);

    authenticate_ed25519(account, signature, auth_ctx, ctx);

    // PTB MUST contain exactly one command.
    assert!(auth_ctx.tx_commands().length() == 1, EInvalidAmountOfCommands);

    let func_key = extract_func_key(&auth_ctx.tx_commands()[0]);
    let store = borrow_store(account);
    assert!(store.is_allowed(&func_key), EUnauthorized);
}
