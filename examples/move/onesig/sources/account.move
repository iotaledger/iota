// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module onesig::account;

use iota::account;
use iota::authenticator_function::AuthenticatorFunctionRefV1;
use iota::dynamic_field;
use iota::ed25519;
use iota::hex::decode;
use onesig::merkle;

#[error(code = 0)]
const EEd25519VerificationFailed: vector<u8> = b"Ed25519 authenticator verification failed.";
#[error(code = 1)]
const EInvalidMerkleProof: vector<u8> = b"Invalid Merkle proof.";

/// Dynamic-field name for the Owner Public Key.
public struct OwnerPublicKey has copy, drop, store {}

/// This struct represents an account which allows to execute several transactions using a single signature.
public struct Account has key {
    id: UID,
}

/// Creates a new `Account` instance as a shared object with the given public key and authenticator.
public fun create(
    public_key: vector<u8>,
    authenticator: AuthenticatorFunctionRefV1<Account>,
    ctx: &mut TxContext,
) {
    let mut account = Account { id: object::new(ctx) };

    dynamic_field::add(&mut account.id, owner_public_key(), public_key);

    account::create_account_v1(account, authenticator);
}

/// Returns the address of the account.
public fun account_address(self: &Account): address {
    self.id.to_address()
}

/// Authenticates a transaction.
/// The signature is verified against the Merkle root, which represents the set of transactions authorized by the account.
/// The Merkle proof is verified against the transaction digest in the transaction context, ensuring that the transaction is part of the authorized set.
#[authenticator]
public fun authenticate(
    account: &Account,
    merkle_root: vector<u8>,
    merkle_proof: vector<vector<u8>>,
    signature: vector<u8>,
    _: &AuthContext,
    ctx: &TxContext,
) {
    verify_merkle_root(account, &merkle_root, &signature);

    verify_merkle_proof(&merkle_root, &merkle_proof, ctx);
}

/// Verify the Merkle root against the provided signature.
/// Ed25519 is used for simplicity. It can be extended to include a set of public keys to verify the signature.
fun verify_merkle_root(self: &Account, root: &vector<u8>, signature: &vector<u8>) {
    assert!(
        ed25519::ed25519_verify(&decode(*signature), self.public_key(), root),
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

/// Helper function to get the dynamic field key for owner public key.
fun owner_public_key(): OwnerPublicKey {
    OwnerPublicKey {}
}

/// Helper function to borrow the owner public key from the account.
fun public_key(self: &Account): &vector<u8> {
    dynamic_field::borrow(&self.id, owner_public_key())
}
