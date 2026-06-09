// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module lean_imt_account::lean_imt_account;

use iota::account;
use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::ed25519;
use lean_imt_account::lean_imt;

// === Errors ===

#[error(code = 0)]
const ETransactionSenderIsNotTheAccount: vector<u8> = b"Transaction must be signed by the account.";
#[error(code = 1)]
const EEd25519VerificationFailed: vector<u8> = b"Ed25519 authenticator verification failed.";

// === Structs ===

/// This struct represents an abstract IOTA account.
/// This account stores a root of a Merkle tree used to store eligible users.
///
/// It holds all the related data as dynamic fields to simplify updates, migrations and extensions.
/// Arbitrary dynamic fields may be added and removed as necessary.
///
/// An `LeanIMTAccount` cannot be constructed directly. To create an `LeanIMTAccount` use `LeanIMTAccountBuilder`.
public struct LeanIMTAccount has key {
    id: UID,
    root: vector<u8>,
}

// === LeanIMTAccount Handling ===

/// Creates a new `LeanIMTAccount` as a shared object with the given authenticator
/// and sets a root.
///
/// The `AuthenticatorFunctionRef` will be attached to the account being built.
public fun create(
    root: vector<u8>,
    authenticator: AuthenticatorFunctionRefV1<LeanIMTAccount>,
    ctx: &mut TxContext,
) {
    let account = LeanIMTAccount { id: object::new(ctx), root };
    account::create_account_v1(account, authenticator);
}

/// Rotates the account root to a new one.
/// This is unsafe as anyone with access to the account could call this function and change
/// the root to an arbitrary value, potentially locking all the users out of the account.
/// An admin cap could be used to limit this.
public fun rotate_root(self: &mut LeanIMTAccount, root: vector<u8>, ctx: &TxContext) {
    // Check that the sender of this transaction is the account.
    ensure_tx_sender_is_account(self, ctx);

    self.root = root;
}

// === Authenticators ===

/// The lean-IMT's leaves are hashes of public keys, so a user can just pass a `leaf` in order to not disclose their
/// main public key. This means that `signing_public_key` is different from the user's main public key and it is
/// only used for securing the MoveAuthenticator (by signing the TX digest).
#[authenticator]
public fun secret_ed25519_authenticator(
    account: &LeanIMTAccount,
    signature: vector<u8>,
    signing_public_key: vector<u8>,
    leaf: vector<u8>,
    pvk: vector<u8>,
    proof_points: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    authenticate_with_secret_ed25519(
        account,
        signature,
        signing_public_key,
        leaf,
        pvk,
        proof_points,
        ctx,
    );
}

/// If the user wants to disclose their public key, a different signing key is not necessary. In this case,
/// the `leaf` can be computed from the public key and passed to the proof verification.
#[authenticator]
public fun public_key_ed25519_authenticator(
    account: &LeanIMTAccount,
    signature: vector<u8>,
    public_key: vector<u8>,
    pvk: vector<u8>,
    proof_points: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    authenticate_with_public_key_ed25519(
        account,
        signature,
        public_key,
        pvk,
        proof_points,
        ctx,
    );
}

// === Public Authenticators Helpers ===

// This function performs the actual authentication logic for `authenticate_with_secret`.
// It checks the signature and then verifies that the provided leaf is part of the lean IMT
//with the given root using the provided proof.
public fun authenticate_with_secret_ed25519(
    account: &LeanIMTAccount,
    signature: vector<u8>,
    signing_public_key: vector<u8>,
    leaf: vector<u8>,
    pvk: vector<u8>,
    proof_points: vector<u8>,
    ctx: &TxContext,
) {
    check_tx_digest_ed25519_signature(signature, signing_public_key, ctx);

    lean_imt::verify_proof(pvk, proof_points, account.root, leaf);
}

// This function performs the actual authentication logic for `authenticate_with_public_key`.
// It checks the signature, then it derives the leaf from the public key and then verifies that
// the leaf is part of the lean IMT with the given root using the provided proof.
public fun authenticate_with_public_key_ed25519(
    account: &LeanIMTAccount,
    signature: vector<u8>,
    public_key: vector<u8>,
    pvk: vector<u8>,
    proof_points: vector<u8>,
    ctx: &TxContext,
) {
    check_tx_digest_ed25519_signature(signature, public_key, ctx);

    let leaf = lean_imt::derive_leaf_from_public_key(public_key);

    lean_imt::verify_proof(pvk, proof_points, account.root, leaf);
}

// === Public-View Functions ===

/// An utility function to borrow the account's root.
public fun root(account: &LeanIMTAccount): vector<u8> {
    account.root
}

/// Return the account's address.
public fun account_address(self: &LeanIMTAccount): address {
    self.id.to_address()
}

// === Admin Functions ===

/// Check that the sender of this transaction is the account.
fun ensure_tx_sender_is_account(self: &LeanIMTAccount, ctx: &TxContext) {
    assert!(self.id.uid_to_address() == ctx.sender(), ETransactionSenderIsNotTheAccount);
}

// === Private Functions ===

// Checks that the signature is valid for the transaction digest and the given public key.
fun check_tx_digest_ed25519_signature(
    signature: vector<u8>,
    signing_public_key: vector<u8>,
    ctx: &TxContext,
) {
    assert!(
        ed25519::ed25519_verify(&signature, &signing_public_key, ctx.digest()),
        EEd25519VerificationFailed,
    );
}
