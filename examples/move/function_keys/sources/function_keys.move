// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Public API & authenticator for per-account Function Keys (allow-set).
///
/// This module provides:
/// - `attach` to initialize the per-account allow-set (a dynamic field).
/// - `create` to create a new `IOTAccount` with a public key and an authenticator.
/// - `grant_permission` / `revoke_permission` admin operations over a per-pubkey allow-set.
/// - `has_permission` read-only query.
/// - `authenticate` dual-flow implementation:
///     1. OWNER FLOW (bypass): if the provided signature verifies against the account owner
///        Ed25519 public key (stored by the underlying account), authentication succeeds **without**
///        enforcing any function key restrictions or command count checks.
///     2. FUNCTION KEY FLOW (delegated): otherwise, we treat `pub_key` as a delegated key:
///        - verify signature against `pub_key`
///        - enforce exactly one PTB command
///        - extract a `FunctionKey` from that sole command and ensure it is allowed for `pub_key`.
///
/// This allows the true account owner to perform arbitrary programmable transactions while
/// enabling granular function-level delegation to other keys.
module function_keys::function_keys;

use function_keys::fk_store::{
    extract_func_key,
    FunctionKey,
    FunctionKeysStore,
    build_fn_keys_store,
    allow,
    disallow,
    is_allowed
};
use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::ed25519;
use iota::hex::decode;
use iotaccount::iotaccount::{builder, ensure_tx_sender_is_account, IOTAccount};

// --------------------
// Errors
// --------------------

/// DF missing (forgot to `create`).
#[error(code = 0)]
const EFunctionKeysNotInitialized: vector<u8> = b"The function key has not been initialized";
/// PTB does not contain **exactly one** command.
#[error(code = 1)]
const EInvalidAmountOfCommands: vector<u8> = b"Invalid number of commands";
/// Called function not in the allow-set.
#[error(code = 2)]
const EUnauthorized: vector<u8> = b"Function key is not the allowed set";
/// Ed225519 verification has failed (delegated flow).
#[error(code = 3)]
const EEd25519VerificationFailed: vector<u8> = b"Ed25519 verification has failed";

/// Dynamic-field name for the Owner Public Key store inside the `IOTAccount`.
public struct OwnerPublicKey has copy, drop, store {}

/// Dynamic-field name for the Function Keys store inside the `IOTAccount`.
public struct FunctionKeysName has copy, drop, store {}

/// Creates a new `IOTAccount` as a shared object with the given authenticator.
public fun create(
    public_key: vector<u8>,
    authenticator: AuthenticatorFunctionRefV1<IOTAccount>,
    ctx: &mut TxContext,
) {
    builder(authenticator, ctx)
        .add_dynamic_field(owner_public_key(), public_key)
        .add_dynamic_field(fk_store_key(), build_fn_keys_store(ctx))
        .build();
}

/// Grants (allows) a `FunctionKey` under a specific `pub_key`.
/// - Only the account owner may mutate their DF.
public fun grant_permission(
    account: &mut IOTAccount,
    pub_key: vector<u8>,
    func_key: FunctionKey,
    ctx: &mut TxContext,
) {
    assert!(account.has_field(fk_store_key()), EFunctionKeysNotInitialized);

    let fk_store = borrow_function_keys_store_mut(account, ctx);
    fk_store.allow(pub_key, func_key);
}

/// Revokes (disallows) a `FunctionKey` under a specific `pub_key`.
public fun revoke_permission(
    account: &mut IOTAccount,
    pub_key: vector<u8>,
    func_key: &FunctionKey,
    ctx: &TxContext,
) {
    assert!(account.has_field(fk_store_key()), EFunctionKeysNotInitialized);

    let fk_store = borrow_function_keys_store_mut(account, ctx);
    fk_store.disallow(pub_key, func_key);
}

/// Read-only query for membership in the per-pubkey allow-set.
public fun has_permission(account: &IOTAccount, pub_key: vector<u8>, func_key: &FunctionKey): bool {
    if (!account.has_field(fk_store_key())) return false;
    let fk_store = borrow_function_keys_store(account);
    fk_store.is_allowed(pub_key, func_key)
}

// --------------------
// Authenticator
// --------------------

/// Dual-flow authenticator
///
/// **Owner flow (bypass):**
/// If `ctx.sender()` equals the account address, we verify the signature against the stored
/// owner public key. If verification succeeds, authentication passes immediately (no Function Keys
/// checks and no command count enforcement).
///
/// **Delegated flow (function-key):**
/// If `ctx.sender()` is not the account address, we treat the provided `pub_key` as a delegated key:
///   1) Verify signature against `pub_key`.
///   2) Require exactly one PTB command.
///   3) Extract `FunctionKey` from that sole command.
///   4) Assert that `func_key` is allowed for `pub_key` in this account’s store.
///
/// Fails with:
/// - `EFunctionKeysNotInitialized` if the store is missing (delegated flow).
/// - `EEd25519VerificationFailed` if signature verification fails (owner or delegated flow).
/// - `EInvalidAmountOfCommands` if the PTB has ≠ 1 command (delegated flow).
/// - `EUnauthorized` if the function is not authorized for the delegated key (delegated flow).
#[authenticator]
public fun authenticate(
    account: &IOTAccount,
    pub_key: vector<u8>,
    signature: vector<u8>,
    auth_ctx: &AuthContext,
    ctx: &TxContext,
) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(account, ctx);
    // Decode signature once for both attempts.
    let sig_bytes = decode(signature);
    // Verify against the stored owner public key.
    let owner_pk = borrow_public_key(account);
    let is_owner = pub_key == owner_pk;
    let is_ed25519_verified = ed25519::ed25519_verify(&sig_bytes, &pub_key, ctx.digest());
    if (is_owner) {
        // OWNER FLOW
        assert!(is_ed25519_verified, EEd25519VerificationFailed);
    } else {
        // FUNCTION KEY FLOW
        assert!(account.has_field(fk_store_key()), EFunctionKeysNotInitialized);
        // Verify delegated signature against provided pub_key.
        assert!(is_ed25519_verified, EEd25519VerificationFailed);

        // Require exactly one command.
        assert!(auth_ctx.tx_commands().length() == 1, EInvalidAmountOfCommands);
        // Extract and check allow-set membership.
        let func_key = extract_func_key(&auth_ctx.tx_commands()[0]);
        let fk_store = borrow_function_keys_store(account);

        assert!(fk_store.is_allowed(pub_key, &func_key), EUnauthorized);
    }
}

public fun borrow_public_key(account: &IOTAccount): &vector<u8> {
    account.borrow_field(owner_public_key())
}

fun borrow_function_keys_store(account: &IOTAccount): &FunctionKeysStore {
    account.borrow_field(fk_store_key())
}

fun borrow_function_keys_store_mut(
    account: &mut IOTAccount,
    ctx: &TxContext,
): &mut FunctionKeysStore {
    account.borrow_field_mut(fk_store_key(), ctx)
}

fun fk_store_key(): FunctionKeysName { FunctionKeysName {} }

fun owner_public_key(): OwnerPublicKey { OwnerPublicKey {} }

/// Creates a new `IOTAccount` as a shared object with the given authenticator, but without
/// attaching the Function Keys store. This is useful for testing purposes.
#[test_only]
public fun create_without_fk_store(
    public_key: vector<u8>,
    authenticator: AuthenticatorFunctionRefV1<IOTAccount>,
    ctx: &mut TxContext,
) {
    builder(authenticator, ctx).add_dynamic_field(owner_public_key(), public_key).build();
}
