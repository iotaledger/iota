// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// The OneSigAccount module defines an account abstraction that allows executing multiple transactions
/// with a single signature using a Merkle tree structure for transaction authorization. It includes
/// functionality for account creation, authentication, and Merkle proof verification.
///
/// The account is created with a public key and an authenticator function. To authenticate the account,
/// the authenticator verifies the provided signature against the Merkle root, which represents the set of
/// authorized transactions. It also verifies that the transaction digest is part of the authorized set
/// using the Merkle proof.
///
/// The implementation of this module is based on the OneSig protocol (https://github.com/LayerZero-Labs/OneSig)
/// and is designed for demonstration purposes only. It can be extended to support more complex authentication
/// schemes, such as multiple signatures or different types of authenticators.
module onesig::account;

use iota::account;
use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::ed25519;
use onesig::merkle;
use public_key_authentication::public_key_authentication;

// === Errors ===

#[error(code = 0)]
const EEd25519VerificationFailed: vector<u8> = b"Ed25519 authenticator verification failed.";
#[error(code = 1)]
const EInvalidMerkleProof: vector<u8> = b"Invalid Merkle proof.";

// === Structs ===

/// This struct represents an account which allows to execute several transactions using a single signature.
public struct OneSigAccount has key {
    id: UID,
}

// === OneSigAccount Handling ===

/// Creates a new `OneSigAccount` instance as a shared object with the given public key and authenticator.
public fun create(
    public_key: vector<u8>,
    authenticator: AuthenticatorFunctionRefV1<OneSigAccount>,
    ctx: &mut TxContext,
) {
    // Create the OneSig account object.
    let mut account = OneSigAccount { id: object::new(ctx) };
    let id = &mut account.id;

    // Attach public key using the public_key_authentication module.
    public_key_authentication::attach_public_key(id, public_key);

    // Finalize account creation.
    account::create_account_v1(account, authenticator);
}

// === Authenticators ===

/// Authenticates a transaction.
/// The signature is verified against the Merkle root, which represents the set of transactions authorized by the account.
/// The Merkle proof is verified against the transaction digest in the transaction context, ensuring that the transaction is part of the authorized set.
#[authenticator]
public fun onesig_authenticator(
    account: &OneSigAccount,
    merkle_root: vector<u8>,
    merkle_proof: vector<vector<u8>>,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    verify_merkle_root(account, &merkle_root, &signature);

    verify_merkle_proof(&merkle_root, &merkle_proof, ctx);
}

// === View Functions ===

/// Returns the address of the account.
public fun account_address(self: &OneSigAccount): address {
    self.id.to_address()
}

/// Helper function to borrow the owner public key from the account.
public fun public_key(account: &OneSigAccount): &vector<u8> {
    public_key_authentication::borrow_public_key(&account.id)
}

// === Private Functions ===

/// Verify the Merkle root against the provided signature.
/// Ed25519 is used for simplicity. It can be extended to include a set of public keys to verify the signature.
fun verify_merkle_root(self: &OneSigAccount, root: &vector<u8>, signature: &vector<u8>) {
    assert!(
        ed25519::ed25519_verify(signature, self.public_key(), root),
        EEd25519VerificationFailed,
    );
}

/// Verify the Merkle proof for the transaction digest.
fun verify_merkle_proof(
    merkle_root: &vector<u8>,
    merkle_proof: &vector<vector<u8>>,
    ctx: &TxContext,
) {
    let leaf_raw = ctx.digest();

    assert!(
        merkle::verify_sorted_keccak_from_leaf_bytes(leaf_raw, merkle_root, merkle_proof),
        EInvalidMerkleProof,
    );
}
