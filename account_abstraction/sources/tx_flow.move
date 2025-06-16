module account_abstraction::tx_flow;

use account_abstraction::account_abstraction::SmartAccount;
use iota::dynamic_object_field as dof;
use iota::ed25519;

const EInvalidSignature: u64 = 3;

/// Holds the raw bytes of the signed transaction with verified signatures
public struct SignedTx has key, store {
    id: UID,
    tx_bytes: vector<u8>,
    verified_signatures: vector<vector<u8>>,
}

/// Holds the raw bytes of the proposed transaction with signatures and their threshold
public struct ProposedTx has key, store {
    id: UID,
    tx_bytes: vector<u8>,
    signatures: vector<vector<u8>>,
    threshold: u64,
}

// Store raw transaction proposal
public fun entry_point(
    smart_account: &mut SmartAccount,
    proposed_tx_bytes: vector<u8>,
    threshold: u64,
    ctx: &mut TxContext,
) {
    let proposed_tx = ProposedTx {
        id: object::new(ctx),
        tx_bytes: proposed_tx_bytes,
        signatures: vector::empty(),
        threshold,
    };
    let proposed_tx_id = *proposed_tx.id.as_inner();
    dof::add(
        smart_account.id_mut(),
        proposed_tx_id,
        proposed_tx,
    );
    assert!(dof::exists_(smart_account.id_mut(), proposed_tx_id), 10);
}

// Signatures verification on-chain
public fun sign_proposed_tx(
    smart_account: &mut SmartAccount,
    tx_id: ID,
    pk: vector<u8>,
    signature: vector<u8>,
    ctx: &mut TxContext,
) {
    let proposed_tx: &mut ProposedTx = dof::borrow_mut(smart_account.id_mut(), tx_id);
    assert!(ed25519::ed25519_verify(&signature, &pk, &proposed_tx.tx_bytes), EInvalidSignature);

    // df::add(
    //   mut proposed_tx.id,
    //   signature.to_string(),
    //   signature
    // );

    //assert!(df::exists_(&proposed_tx.id, signature.to_string()), 11);
    proposed_tx.signatures.push_back(signature);

    if (proposed_tx.threshold == proposed_tx.signatures.length()) {
        let signed_tx = SignedTx {
            id: object::new(ctx),
            tx_bytes: proposed_tx.tx_bytes,
            verified_signatures: proposed_tx.signatures,
        };
        let signed_tx_id = *signed_tx.id.as_inner();
        dof::add(
            smart_account.id_mut(),
            signed_tx_id,
            signed_tx,
        );
        let ProposedTx { id, tx_bytes: _, signatures: _, threshold: _ } = dof::remove(
            smart_account.id_mut(),
            tx_id,
        );
        id.delete();
    }
}
