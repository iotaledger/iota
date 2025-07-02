// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module account_abstraction::tx_flow;

use account_abstraction::smart_account::SmartAccount;
use iota::dynamic_object_field as dof;
use iota::ed25519;
use iota::hex::decode;
use iota::vec_set::{Self, VecSet};

const EInvalidSignature: u64 = 3;

/// Holds the raw bytes of the signed transaction with verified signatures
public struct SignedTx has key, store {
    id: UID,
    tx_digest: vector<u8>,
    tx_bytes: vector<u8>,
    verified_signatures: VecSet<vector<u8>>,
}

/// Holds the raw bytes of the proposed transaction with signatures and their threshold
/// Currently, we store both `tx_digest` and `tx_bytes`:
/// - `tx_digest` is needed to verify the signature
/// - `tx_bytes` is required to execute the transaction later
/// Theoretically, we could extract the digest from tx_bytes on-chain, but we’re skipping this step for the prototype.
public struct ProposedTx has key, store {
    id: UID,
    tx_digest: vector<u8>,
    tx_bytes: vector<u8>,
    signatures: VecSet<vector<u8>>,
    threshold: u64,
}

// Store raw transaction proposal
public fun entry_point(
    smart_account: &mut SmartAccount,
    proposed_tx_digest: vector<u8>,
    proposed_tx_bytes: vector<u8>,
    threshold: u64,
    ctx: &mut TxContext,
) {
    let proposed_tx = ProposedTx {
        id: object::new(ctx),
        tx_digest: proposed_tx_digest, // The digest could be also derived on-chain
        tx_bytes: proposed_tx_bytes,
        signatures: vec_set::empty(),
        threshold,
    };
    let proposed_tx_id = *proposed_tx.id.as_inner();
    dof::add(
        smart_account.id_mut(),
        proposed_tx_id,
        proposed_tx,
    );
}

// Signatures verification on-chain
public fun sign_proposed_tx(
    smart_account: &mut SmartAccount,
    tx_id: ID,
    pk: vector<u8>,
    pure_signature: vector<u8>,
    ctx: &mut TxContext,
) {
    let proposed_tx: &mut ProposedTx = dof::borrow_mut(smart_account.id_mut(), tx_id);
    assert!(
        ed25519::ed25519_verify(&decode(pure_signature), &pk, &proposed_tx.tx_digest),
        EInvalidSignature,
    );

    proposed_tx.signatures.insert(pure_signature);

    if (proposed_tx.threshold == proposed_tx.signatures.size()) {
        let signed_tx = SignedTx {
            id: object::new(ctx),
            tx_digest: proposed_tx.tx_digest,
            tx_bytes: proposed_tx.tx_bytes,
            verified_signatures: proposed_tx.signatures,
        };
        let signed_tx_id = *signed_tx.id.as_inner();
        dof::add(
            smart_account.id_mut(),
            signed_tx_id,
            signed_tx,
        );
        let ProposedTx { id, tx_digest: _, tx_bytes: _, signatures: _, threshold: _ } = dof::remove(
            smart_account.id_mut(),
            tx_id,
        );
        id.delete();
    }
}
