// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// An IOTAccount that enforces a strict "read-only entry-function" policy,
/// demonstrating three enriched `AuthContext` fields added in the Richer Authentication update.
///
/// # Authentication flow
///
/// 1. Verify the Ed25519 `signature` over the transaction digest.
/// 2. Walk `auth_ctx.enriched_tx_commands()`:
///    - For every `MoveCall` assert `is_entry == true` (must be public entry).
///    - For every `MoveCall` assert `returns.is_empty()` (must be void).
/// 3. Walk `auth_ctx.enriched_tx_inputs()`:
///    - For every `ImmOrOwnedObject` input assert `mutable == false`
///      (no `&mut T` object arguments allowed).
///
module entry_only_account::entry_only_account;

use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::ed25519;
use iotaccount::iotaccount::{Self, IOTAccount, IOTAccountBuilder};
use public_key_authentication::public_key_iotaccount;

/// Allows calling `.with_public_key` on an `IOTAccountBuilder`.
use fun public_key_iotaccount::with_public_key as IOTAccountBuilder.with_public_key;

/// Allows calling `.borrow_public_key` on an `IOTAccount`.
use fun public_key_iotaccount::borrow_public_key as IOTAccount.borrow_public_key;

// === Errors ===

/// The Ed25519 signature did not verify against the stored public key.
#[error(code = 0)]
const EEd25519VerificationFailed: vector<u8> = b"Ed25519 verification failed";

/// A MoveCall command targets a function that is not marked `entry`.
/// Checked via the enriched `EnrichedProgrammableMoveCall.is_entry` field.
#[error(code = 1)]
const ENonEntryFunctionCall: vector<u8> =
    b"Only entry functions may be called from this account";

/// A MoveCall command targets a function that returns one or more values.
/// Checked via the enriched `EnrichedProgrammableMoveCall.returns` field.
#[error(code = 2)]
const EFunctionMustBeVoid: vector<u8> =
    b"Called function must not return any values";

/// An object input is passed as mutable (`&mut T`).
/// Checked via the enriched `ImmOrOwnedObjectArg.mutable` field.
#[error(code = 3)]
const ENoMutableObjectInputs: vector<u8> =
    b"Mutable object inputs are not allowed for this account";

// === Account creation ===

/// Create and share a new `IOTAccount` protected by `entry_only_account::authenticate`.
public fun create(
    public_key: vector<u8>,
    authenticator: AuthenticatorFunctionRefV1<IOTAccount>,
    ctx: &mut TxContext,
) {
    iotaccount::builder(authenticator, ctx)
        .with_public_key(public_key)
        .build();
}

// === Authenticator ===

/// Authenticate a transaction for this account.
///
/// Uses three enriched `AuthContext` fields to enforce the read-only
/// entry-function policy described in the module doc.
#[authenticator]
public fun authenticate(
    account: &IOTAccount,
    signature: vector<u8>,
    auth_ctx: &AuthContext,
    ctx: &TxContext,
) {
    // ── 1. Ed25519 signature check ────────────────────────────────────────
    let pub_key = account.borrow_public_key();
    assert!(
        ed25519::ed25519_verify(&signature, pub_key, ctx.digest()),
        EEd25519VerificationFailed,
    );

    // ── 2. Command checks ─────────────────────────────────────────────────
    // Walk every PTB command. For MoveCall variants inspect the two
    // enriched fields that the VM resolved before calling this authenticator.
    let commands = auth_ctx.enriched_tx_commands();
    let mut i = 0;
    while (i < commands.length()) {
        let command = &commands[i];
        if (command.is_move_call()) {
            let call = command.as_move_call().destroy_some();

            // Field 1: `is_entry` — true when the called function
            // is declared `entry` in its module.
            assert!(call.is_entry(), ENonEntryFunctionCall);

            // Field 2: `returns` — list of canonical return types.
            // An empty vector means the function is void.
            assert!(call.returns().is_empty(), EFunctionMustBeVoid);
        };
        i = i + 1;
    };

    // ── 3. Input checks ───────────────────────────────────────────────────
    // Walk every PTB input. For owned/immutable object inputs inspect the
    // enriched `mutable` flag (true when the called function takes `&mut T`).
    let inputs = auth_ctx.enriched_tx_inputs();
    let mut j = 0;
    while (j < inputs.length()) {
        let input = &inputs[j];
        // Field 3: `ImmOrOwnedObjectArg.mutable` — true when
        // the object is passed as a mutable reference (`&mut T`).
        if (input.is_imm_or_owned_object()) {
            let imm_or_owned = input.as_imm_or_owned_object().destroy_some();
            assert!(!imm_or_owned.mutable(), ENoMutableObjectInputs);
        };
        j = j + 1;
    }
}

// === View helpers ===

/// Returns the type names of all `Pure` inputs in the given auth context.
///
/// The `type_name` field is the canonical Move type resolved by the VM at
/// auth time.  This helper demonstrates how `pure_type_name()` can be used
/// to inspect or log the types of pure inputs.
public fun pure_input_types(auth_ctx: &AuthContext): vector<std::type_name::TypeName> {
    let inputs = auth_ctx.enriched_tx_inputs();
    let mut type_names = vector::empty<std::type_name::TypeName>();
    let mut i = 0;
    while (i < inputs.length()) {
        let maybe_type = inputs[i].pure_type_name();
        if (maybe_type.is_some()) {
            type_names.push_back(maybe_type.destroy_some());
        };
        i = i + 1;
    };
    type_names
}
