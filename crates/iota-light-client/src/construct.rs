// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Result, anyhow, bail};
use iota_types::{
    effects::TransactionEffectsAPI,
    full_checkpoint_content::{CheckpointData, CheckpointTransaction},
};

use crate::proof::{Proof, ProofTargets, TransactionProof};

/// Construct a proof from the given checkpoint data and proof targets.
///
/// Only minimal cheaper checks are performed to ensure the proof is valid. If
/// you need guaranteed validity consider calling `verify_proof` function on the
/// constructed proof. It either returns `Ok` with a proof, or `Err` with a
/// description of the error.
pub fn construct_proof(targets: ProofTargets, checkpoint: &CheckpointData) -> Result<Proof> {
    let checkpoint_summary = checkpoint.checkpoint_summary.clone();
    let mut proof = Proof {
        targets,
        checkpoint_summary,
        contents_proof: None,
    };

    // Do a minimal check that the given checkpoint data is consistent with the
    // committee
    if let Some(committee_target) = &proof.targets.committee {
        // Check we have the correct epoch
        if proof.checkpoint_summary.epoch() + 1 != committee_target.epoch {
            bail!("Epoch mismatch between checkpoint and committee");
        }

        // Check its an end of epoch checkpoint
        if proof.checkpoint_summary.end_of_epoch_data.is_none() {
            bail!("Expected end of epoch checkpoint");
        }
    }

    // If proof targets include objects or events, we need to include the contents
    // proof Need to ensure that all targets refer to the same transaction first
    // of all
    let object_tx = proof
        .targets
        .objects
        .iter()
        .map(|(_, o)| o.previous_transaction);
    let event_tx = proof.targets.events.iter().map(|(eid, _)| eid.tx_digest);
    let mut all_tx = object_tx.chain(event_tx);

    // Get the first tx ID
    let target_tx_id = if let Some(first_tx) = all_tx.next() {
        first_tx
    } else {
        // Since there is no target we just return the summary proof
        return Ok(proof);
    };

    // Basic check that all targets refer to the same transaction
    if !all_tx.all(|tx| tx == target_tx_id) {
        bail!("All targets must refer to the same transaction");
    }

    // Find the transaction in the checkpoint data
    let tx = checkpoint
        .transactions
        .iter()
        .find(|t| t.effects.transaction_digest() == &target_tx_id)
        .ok_or_else(|| anyhow!("Transaction not found in checkpoint data"))?
        .clone();

    let CheckpointTransaction {
        transaction,
        effects,
        events,
        ..
    } = tx;

    // Add all the transaction data in there
    proof.contents_proof = Some(TransactionProof {
        checkpoint_contents: checkpoint.checkpoint_contents.clone(),
        transaction,
        effects,
        events,
    });

    // TODO: should we check that the objects & events are in the transaction, to
    //       avoid constructing invalid proofs? I opt to not check because the check
    //       is expensive (sequential scan of all objects).

    Ok(proof)
}
