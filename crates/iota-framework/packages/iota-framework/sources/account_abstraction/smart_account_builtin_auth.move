// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Built-in authenticator support for `SmartAccount`: the built-in authenticator for IOTA's
/// standard signature schemes (Ed25519, Secp256k1, Secp256r1, MultiSig, Passkey), checking
/// signatures against the account's public key.
///
/// This is the only module that attaches, rotates or detaches a `SmartAccount`'s public key, and
/// it keeps the key and the built-in authenticator together: an account has both or neither.
///
/// - `builder_v1` and the `ClaimAccount` pipeline create an account with both;
/// - `rotate_to_builtin_auth_v1` attaches a key and sets the built-in authenticator in one call;
/// - `rotate_to_custom_auth_v1` replaces the built-in authenticator with a custom one and detaches
///   the key in one call;
/// - `rotate_public_key` replaces the key while the built-in authenticator is set. The built-in
///   authenticator uses the scheme of the attached key, so it stays as it is.
///
/// `smart_account::build_v1` and `smart_account::rotate_auth_function_ref_v1` check the same
/// pairing, so no `SmartAccount` can have the built-in authenticator without the key, or the key
/// without the built-in authenticator.
///
/// `builder_v1`, `rotate_to_builtin_auth_v1` and `rotate_public_key` attach a key only with its
/// holder's proof: a signature by the key, as an IOTA personal message, over a text that names
/// the network, the account the key goes to, whether that account is new, the transaction's
/// nonce, and the last epoch in which the proof is valid. For example:
///
/// ```
/// check: 0x2::smart_account_builtin_auth::verify_key_proof
/// network: 2304aa976b1765b020886e8b5c7369e169bc779154b9c3efb4dd15c6039c0679
/// account: 0xf9e4f7f66304ca332d20636b5358b0b9297afbc7428bc107fc5dbc64fb204125
/// new account: true
/// nonce: dbf7a1f60789ef83fe154a99c04c583785f44a74760105e17a8e7338c77c6cd1
/// valid until epoch: 730
/// ```
///
/// A proof is accepted only in the one executed transaction whose nonce it names, and in that
/// transaction by every check for the same account. A MultiSig key can't sign the proof, so it
/// reaches a `SmartAccount` only through `ClaimAccount`.
///
/// Claiming an existing address through the `ClaimAccount` transaction kind drives the private
/// `claim_account_v1` below. It needs no proof: the key signs that transaction.
module iota::smart_account_builtin_auth;

use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::builtin_authenticator_functions::{Self, builtin_authenticator_function_ref_v1};
use iota::public_key::PublicKey;
use iota::smart_account::{Self, SmartAccount, SmartAccountBuilder};

// === Errors ===

#[error(code = 0)]
const EBuiltinAuthAlreadySet: vector<u8> =
    b"The built-in authenticator is already set; use `rotate_public_key` to replace its key.";
#[error(code = 1)]
const EBuiltinAuthNotSet: vector<u8> = b"The built-in authenticator is not set.";
#[error(code = 2)]
const EAuthenticatorIsBuiltin: vector<u8> =
    b"Rotate to the built-in authenticator with `rotate_to_builtin_auth_v1`.";
#[error(code = 3)]
const EKeyProofExpired: vector<u8> = b"The key proof has expired.";
#[error(code = 4)]
const EInvalidKeyProof: vector<u8> = b"The key proof is not a valid signature by the key.";

// === Public Functions ===

/// Creates a `SmartAccountBuilder` for a new account backed by the built-in authenticator, with
/// `public_key` attached.
///
/// `key_proof` is the key holder's proof (see the module documentation), for a new account
/// created by the transaction sender. It is in the flag-prefixed wire format of the key's scheme.
///
/// Finish it with `smart_account::build_v1`.
///
/// Emits a `builtin_authenticator_functions::PublicKeyAttached` event on success.
///
/// Aborts if the current epoch is later than `valid_until_epoch`.
/// Aborts if `key_proof` is not a valid proof by `public_key`.
/// Aborts if `enable_builtin_move_authenticators` or `enable_smart_accounts` is not enabled in the
/// protocol config.
public fun builder_v1(
    public_key: PublicKey,
    key_proof: vector<u8>,
    valid_until_epoch: u64,
    ctx: &mut TxContext,
): SmartAccountBuilder {
    verify_key_proof(&public_key, &key_proof, ctx.sender(), true, valid_until_epoch, ctx);

    let builder = smart_account::builder_v1(builtin_authenticator_function_ref_v1(), ctx);
    with_public_key(builder, public_key)
}

// === View Functions ===

/// Returns `true` if and only if the account's authenticator is the built-in one.
public fun has_builtin_auth(account: &SmartAccount): bool {
    builtin_authenticator_functions::is_builtin_authenticator(
        account.borrow_auth_function_ref_v1(),
    )
}

/// Returns `true` if and only if the account has a public key attached.
public fun has_public_key(account: &SmartAccount): bool {
    builtin_authenticator_functions::has_public_key(account.uid())
}

/// Borrows the public key attached to the account.
///
/// Aborts if no public key is attached.
public fun borrow_public_key(account: &SmartAccount): &PublicKey {
    builtin_authenticator_functions::borrow_public_key(account.uid())
}

// === Admin Functions ===

/// Attaches `public_key` to the account and rotates its authenticator to the built-in one, which
/// checks signatures against that key. Returns the previous authenticator.
///
/// `key_proof` is the key holder's proof (see the module documentation), for this existing
/// account. It is in the flag-prefixed wire format of the key's scheme.
///
/// Emits a `builtin_authenticator_functions::PublicKeyAttached` event and an
/// `account::AuthenticatorFunctionRefV1Rotated` event on success.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if the built-in authenticator is already set; use `rotate_public_key` to replace its
/// key.
/// Aborts if the current epoch is later than `valid_until_epoch`.
/// Aborts if `key_proof` is not a valid proof by `public_key`.
/// Aborts if `enable_builtin_move_authenticators` is not enabled in the protocol config.
public fun rotate_to_builtin_auth_v1(
    account: &mut SmartAccount,
    public_key: PublicKey,
    key_proof: vector<u8>,
    valid_until_epoch: u64,
    ctx: &TxContext,
): AuthenticatorFunctionRefV1<SmartAccount> {
    account.ensure_tx_sender_is_smart_account(ctx);
    assert!(!has_builtin_auth(account), EBuiltinAuthAlreadySet);
    verify_key_proof(
        &public_key,
        &key_proof,
        account.account_address(),
        false,
        valid_until_epoch,
        ctx,
    );

    builtin_authenticator_functions::attach_public_key(account.uid_mut(), public_key);
    account.rotate_auth_function_ref_v1(builtin_authenticator_function_ref_v1(), ctx)
}

/// Rotates the account's authenticator from the built-in one to the custom `authenticator`,
/// detaching the public key, and returns that key.
///
/// To rotate from one custom authenticator to another, use
/// `smart_account::rotate_auth_function_ref_v1`.
///
/// Emits a `builtin_authenticator_functions::PublicKeyDetached` event and an
/// `account::AuthenticatorFunctionRefV1Rotated` event on success.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if `authenticator` is the built-in one; use `rotate_to_builtin_auth_v1`.
/// Aborts if the built-in authenticator is not set; use
/// `smart_account::rotate_auth_function_ref_v1`.
public fun rotate_to_custom_auth_v1(
    account: &mut SmartAccount,
    authenticator: AuthenticatorFunctionRefV1<SmartAccount>,
    ctx: &TxContext,
): PublicKey {
    account.ensure_tx_sender_is_smart_account(ctx);
    assert!(
        !builtin_authenticator_functions::is_builtin_authenticator(&authenticator),
        EAuthenticatorIsBuiltin,
    );
    assert!(has_builtin_auth(account), EBuiltinAuthNotSet);

    let public_key = builtin_authenticator_functions::detach_public_key(account.uid_mut());
    account.rotate_auth_function_ref_v1(authenticator, ctx);
    public_key
}

/// Replaces the account's public key with `public_key` and returns the previous key. The built-in
/// authenticator stays as it is and checks signatures against the new key.
///
/// `key_proof` is the proof of the new key's holder (see the module documentation), for this
/// existing account. It is in the flag-prefixed wire format of the key's scheme.
///
/// Emits a `builtin_authenticator_functions::PublicKeyRotated` event on success.
///
/// Aborts if the transaction sender is not the account.
/// Aborts if the built-in authenticator is not set; use `rotate_to_builtin_auth_v1`.
/// Aborts if the current epoch is later than `valid_until_epoch`.
/// Aborts if `key_proof` is not a valid proof by `public_key`.
public fun rotate_public_key(
    account: &mut SmartAccount,
    public_key: PublicKey,
    key_proof: vector<u8>,
    valid_until_epoch: u64,
    ctx: &TxContext,
): PublicKey {
    account.ensure_tx_sender_is_smart_account(ctx);
    assert!(has_builtin_auth(account), EBuiltinAuthNotSet);
    verify_key_proof(
        &public_key,
        &key_proof,
        account.account_address(),
        false,
        valid_until_epoch,
        ctx,
    );

    builtin_authenticator_functions::rotate_public_key(account.uid_mut(), public_key)
}

// === Private Functions ===

/// Claims the sender's address and creates a `SmartAccount` at it, backed by the built-in
/// authenticator with `public_key` attached.
///
/// This is the whole `ClaimAccount` pipeline. It is **private on purpose**: the account object
/// it creates has an ID equal to a signature-derivable address, so `ClaimAccount` must stay the
/// one and only way such an object can come into existence. A private function is reachable from
/// the node's own PTB, which runs in an execution mode that bypasses visibility, and from nowhere
/// else — not from a user PTB, and not from another package, which could otherwise wrap a public
/// entry point. See the `iota::clock::consensus_commit_prologue` function for the same idiom.
///
/// Emits a `builtin_authenticator_functions::PublicKeyAttached` event and an
/// `account::MutableAccountCreated` event.
///
/// Aborts if `public_key` does not derive the sender's address.
/// Aborts if `enable_builtin_move_authenticators` or `enable_smart_accounts` is not enabled in the
/// protocol config.
#[allow(unused_function)]
fun claim_account_v1(public_key: PublicKey, ctx: &TxContext) {
    let builder = smart_account::claim_builder(
        public_key,
        builtin_authenticator_function_ref_v1(),
        ctx,
    );
    with_public_key(builder, public_key).build_v1();
}

/// Attaches `public_key` to the account being built.
fun with_public_key(mut builder: SmartAccountBuilder, public_key: PublicKey): SmartAccountBuilder {
    builtin_authenticator_functions::attach_public_key(builder.account_uid_mut(), public_key);
    builder
}

/// Aborts unless `signature` is a proof by `public_key` for attaching it to `account`.
fun verify_key_proof(
    public_key: &PublicKey,
    signature: &vector<u8>,
    account: address,
    new_account: bool,
    valid_until_epoch: u64,
    ctx: &TxContext,
) {
    assert!(ctx.epoch() <= valid_until_epoch, EKeyProofExpired);

    let message = key_proof_message(
        ctx.chain_identifier(),
        account,
        new_account,
        ctx.tx_nonce(),
        valid_until_epoch,
    );
    assert!(public_key.verify_personal_message_signature(signature, &message), EInvalidKeyProof);
}

/// Returns the UTF-8 bytes of the text that a key holder signs, as an IOTA personal message, to
/// approve attaching their key to `account`. For a new account, `account` is the transaction
/// sender. The SDK function that clients call writes the same text.
///
/// Aborts with code 5 if `chain_identifier` or `nonce` is not 32 bytes.
native fun key_proof_message(
    chain_identifier: vector<u8>,
    account: address,
    new_account: bool,
    nonce: vector<u8>,
    valid_until_epoch: u64,
): vector<u8>;

// === Test Functions ===

/// Test-only entry to `key_proof_message`, which only `verify_key_proof` calls.
#[test_only]
public fun key_proof_message_for_testing(
    chain_identifier: vector<u8>,
    account: address,
    new_account: bool,
    nonce: vector<u8>,
    valid_until_epoch: u64,
): vector<u8> {
    key_proof_message(chain_identifier, account, new_account, nonce, valid_until_epoch)
}

/// Test-only entry to `claim_account_v1`, which is private so that only the node's
/// `ClaimAccount` pipeline can reach it.
#[test_only]
public fun claim_account_v1_for_testing(public_key: PublicKey, ctx: &TxContext) {
    claim_account_v1(public_key, ctx)
}
