// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// The IOTAccount with FunctionCallKeys defines an account that can be used to allow function-level
/// delegation through the usage of function call keys. An owner controls the account, while different
/// users can be granted permissions to call specific functions through the usage of function call keys.
///
/// This module provides:
/// - `attach` to initialize the per-account allow-set (a dynamic field).
/// - `create` to create a new `IOTAccount` with a public key and an authenticator.
/// - `grant_permission` / `revoke_permission` admin operations over a per-pubkey allow-set.
/// - `has_permission` read-only query.
/// - `authenticate` dual-flow implementation:
///     1. OWNER FLOW (bypass): if the provided signature verifies against the account owner
///        Ed25519 public key (stored by the underlying account), authentication succeeds **without**
///        enforcing any function call key restrictions or command count checks.
///     2. FUNCTION CALL KEY FLOW (delegated): otherwise, we treat `pub_key` as a delegated key:
///        - verify signature against `pub_key`
///        - enforce exactly one PTB command
///        - extract a `FunctionRef` from that sole command and ensure it is allowed for `pub_key`.
///
/// This allows the true account owner to perform arbitrary programmable transactions while
/// enabling granular function-level delegation to other keys.
module function_call_keys::function_call_keys;

use function_call_keys::function_call_keys_store::{
    Self,
    FunctionRef,
    FunctionCallKeysStore,
    allow,
    disallow,
    is_allowed,
    function_call_keys_store_field_name
};
use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::ed25519;
use iota::ptb_command::Command;
use iotaccount::iotaccount::{Self, IOTAccountBuilder, IOTAccount};
use public_key_authentication::public_key_iotaccount;

/// Allows calling `.with_public_key` on an `IOTAccountBuilder` to set a `public_key`.
use fun public_key_iotaccount::with_public_key as IOTAccountBuilder.with_public_key;

/// Allows calling `.borrow_public_key` on an `IOTAccountBuilder` to set a `public_key`.
use fun public_key_iotaccount::borrow_public_key as IOTAccount.borrow_public_key;

/// Allows calling `.has_permission` on an `IOTAccountBuilder` to set a `public_key`.
use fun has_permission as IOTAccount.has_permission;

/// Allows calling `.extract_function_ref` on a `Command` to extract a `FunctionRef`.
use fun function_call_keys_store::extract_function_ref as Command.extract_function_ref;

// === Errors ===

/// DF missing (forgot to `create`).
#[error(code = 0)]
const EFunctionCallKeysNotInitialized: vector<u8> =
    b"The function call key has not been initialized";
/// PTB does not contain **exactly one** command.
#[error(code = 1)]
const EInvalidAmountOfCommands: vector<u8> = b"Invalid number of commands";
/// Called function not in the allow-set.
#[error(code = 2)]
const EUnauthorized: vector<u8> = b"Function call key is not the allowed set";
/// Ed225519 verification has failed (delegated flow).
#[error(code = 3)]
const EEd25519VerificationFailed: vector<u8> = b"Ed25519 verification has failed";

// === Structs ===

// === IOTAccount with FunctionCallKeys Handling ===

/// Create an IOTAccount with a FunctionCallKeysStore.
///
/// The generated account is first protected by an
/// Ed25519 authentication and then by store of FunctionCallKeys.
/// The provided `public_key` will be used for Ed25519 authentication of the owner of the account.
/// While the `FunctionCallKeysStore` will be used to manage function-level permissions for delegated keys.
public fun create(
    public_key: vector<u8>,
    admin: Option<address>,
    authenticator: AuthenticatorFunctionRefV1<IOTAccount>,
    ctx: &mut TxContext,
) {
    // Create builder and attach the public key and the FunctionCallKeysStore field to the account.
    let builder = iotaccount::builder(authenticator, ctx)
        .with_public_key(public_key)
        .with_field(
            function_call_keys_store_field_name(),
            function_call_keys_store::build(ctx),
        );
    // Optionally attach the admin
    let builder = if (admin.is_some()) {
        builder.with_admin(admin.destroy_some())
    } else {
        builder
    };

    // Finally, build the account and share it.
    builder.build();
}

/// Attach a FunctionCallKeysStore as a dynamic field to the account being built.
public fun with_function_call_keys_store(
    builder: IOTAccountBuilder,
    function_call_keys_store: FunctionCallKeysStore,
): IOTAccountBuilder {
    builder.with_field(function_call_keys_store_field_name(), function_call_keys_store)
}

// === Authenticators ===

/// Dual-flow authenticator
///
/// **Owner flow (bypass):**
/// If `ctx.sender()` equals the account address, we verify the signature against the stored
/// owner public key. If verification succeeds, authentication passes immediately (no Function Call Keys
/// checks and no command count enforcement).
///
/// **Delegated flow (function-call-key):**
/// If `ctx.sender()` is not the account address, we treat the provided `pub_key` as a delegated key:
///   1) Verify signature against `pub_key`.
///   2) Require exactly one PTB command.
///   3) Extract `FunctionRef` from that sole command.
///   4) Assert that `function_ref` is allowed for `pub_key` in this account’s store.
///
/// Fails with:
/// - `EFunctionCallKeysNotInitialized` if the store is missing (delegated flow).
/// - `EEd25519VerificationFailed` if signature verification fails (owner or delegated flow).
/// - `EInvalidAmountOfCommands` if the PTB has ≠ 1 command (delegated flow).
/// - `EUnauthorized` if the function is not authorized for the delegated key (delegated flow).
#[authenticator]
public fun ed25519_authenticator(
    account: &IOTAccount,
    pub_key: vector<u8>,
    signature: vector<u8>,
    auth_ctx: &AuthContext,
    ctx: &TxContext,
) {
    // Verify against the stored owner public key.
    let owner_pk = account.borrow_public_key();
    let is_owner = pub_key == owner_pk;
    let is_ed25519_verified = ed25519::ed25519_verify(&signature, &pub_key, ctx.digest());
    if (is_owner) {
        // OWNER FLOW
        assert!(is_ed25519_verified, EEd25519VerificationFailed);
    } else {
        // FUNCTION CALL KEY FLOW
        assert!(
            account.has_field(function_call_keys_store_field_name()),
            EFunctionCallKeysNotInitialized,
        );
        // Verify delegated signature against provided pub_key.
        assert!(is_ed25519_verified, EEd25519VerificationFailed);

        // Require exactly one command.
        assert!(auth_ctx.tx_commands().length() == 1, EInvalidAmountOfCommands);
        let command = &auth_ctx.tx_commands()[0];
        let function_ref = command.extract_function_ref();

        // Check allow-set membership.
        assert!(account.has_permission(pub_key, &function_ref), EUnauthorized);
    }
}

// === IOTAccount FunctionCallKeysStore Modification Functions ===

/// Grants (allows) a `FunctionRef` under a specific `pub_key`.
/// Only the account owner can mutate this field.
public fun grant_permission(
    account: &mut IOTAccount,
    pub_key: vector<u8>,
    function_ref: FunctionRef,
    ctx: &TxContext,
) {
    assert!(
        account.has_field(function_call_keys_store_field_name()),
        EFunctionCallKeysNotInitialized,
    );

    let function_call_keys_store = account.borrow_field_mut<_, FunctionCallKeysStore>(
        function_call_keys_store_field_name(),
        ctx,
    );
    function_call_keys_store.allow(pub_key, function_ref);
}

/// Revokes (disallows) a `FunctionRef` under a specific `pub_key`.
/// Only the account owner can mutate this field.
public fun revoke_permission(
    account: &mut IOTAccount,
    pub_key: vector<u8>,
    function_ref: &FunctionRef,
    ctx: &TxContext,
) {
    assert!(
        account.has_field(function_call_keys_store_field_name()),
        EFunctionCallKeysNotInitialized,
    );

    let function_call_keys_store = account.borrow_field_mut<_, FunctionCallKeysStore>(
        function_call_keys_store_field_name(),
        ctx,
    );
    function_call_keys_store.disallow(pub_key, function_ref);
}

// === View Functions ===

/// Read-only query for membership in the per-pubkey allow-set.
public fun has_permission(
    account: &IOTAccount,
    pub_key: vector<u8>,
    function_ref: &FunctionRef,
): bool {
    if (!account.has_field(function_call_keys_store_field_name())) return false;

    let function_call_keys_store = account.borrow_field<_, FunctionCallKeysStore>(
        function_call_keys_store_field_name(),
    );
    function_call_keys_store.is_allowed(pub_key, function_ref)
}
