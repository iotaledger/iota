// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::{Arc, OnceLock};

use iota_swarm_config::test_utils::{CommitteeFixture, empty_contents};
use iota_types::{
    effects::TransactionEffectsAPI,
    full_checkpoint_content::{CheckpointData, CheckpointTransaction},
    messages_checkpoint::CertifiedCheckpointSummary,
    storage::ApplyCheckpointResults,
};

use crate::{
    authority::AuthorityState, checkpoint_results_applier::CheckpointResultsApplier,
    transaction_outputs::tests::execute_transfer_both_ways_on,
};

/// Whether the store has recorded effects for `digest`, which is what the
/// checkpoint executor reads to decide a transaction needs no execution.
fn is_executed(authority: &AuthorityState, digest: &iota_sdk_types::TransactionDigest) -> bool {
    authority
        .get_transaction_cache_reader()
        .is_tx_already_executed(digest)
}

/// A certified summary reporting `epoch`, paired below with the transactions
/// the applier reads.
///
/// The committee fixture only builds a root checkpoint at epoch 0, so the
/// epoch is set afterwards and the signatures no longer match the summary.
/// That is fine here: the applier reads only `epoch()` and `sequence_number`,
/// because state sync verifies the signatures and `contents_digest` before it
/// ever hands a checkpoint over.
fn certified_summary(epoch: u64) -> CertifiedCheckpointSummary {
    let fixture = CommitteeFixture::generate(rand::rngs::OsRng, 0, 4);
    let (checkpoints, _, _, _) = fixture.make_checkpoints(1, None, empty_contents);
    let root: CertifiedCheckpointSummary = checkpoints.into_iter().next().unwrap().into();
    let (mut summary, signatures) = root.into_data_and_sig();
    summary.epoch = epoch;
    CertifiedCheckpointSummary::new_from_data_and_sig(summary, signatures)
}

fn checkpoint_data(epoch: u64, transactions: Vec<CheckpointTransaction>) -> CheckpointData {
    CheckpointData {
        checkpoint_summary: certified_summary(epoch),
        checkpoint_contents: empty_contents().into_inner().into_checkpoint_contents(),
        transactions,
    }
}

async fn applier_with_transfer() -> (
    Arc<AuthorityState>,
    CheckpointResultsApplier,
    CheckpointTransaction,
) {
    let (authority, _, checkpoint_tx) = execute_transfer_both_ways_on().await;
    let state = Arc::new(OnceLock::new());
    state
        .set(authority.clone())
        .ok()
        .expect("the cell is fresh");
    let applier = CheckpointResultsApplier::new(state, authority.get_cache_writer().clone());
    (authority, applier, checkpoint_tx)
}

/// Applying a checkpoint's results must record the effects, so the checkpoint
/// executor finds the transaction already executed and skips it.
#[tokio::test]
async fn applies_results_so_the_executor_skips_the_transaction() {
    let (authority, applier, checkpoint_tx) = applier_with_transfer().await;
    let tx_digest = *checkpoint_tx.effects.transaction_digest();
    let epoch = authority.epoch_store_for_testing().epoch();

    let applied = applier
        .try_apply_checkpoint_results(&checkpoint_data(epoch, vec![checkpoint_tx]))
        .expect("verified results must apply");

    assert!(applied, "a checkpoint in the current epoch must be applied");
    assert!(
        is_executed(&authority, &tx_digest),
        "the executor decides what to skip from the recorded effects digest"
    );
}

/// Object markers and shared version assignments are keyed by epoch, so a
/// checkpoint from a different epoch than the node's current one must be left
/// for the executor rather than written under the wrong epoch.
#[tokio::test]
async fn leaves_checkpoints_from_another_epoch_to_the_executor() {
    let (authority, applier, checkpoint_tx) = applier_with_transfer().await;
    let tx_digest = *checkpoint_tx.effects.transaction_digest();
    let current_epoch = authority.epoch_store_for_testing().epoch();
    let other_epoch = current_epoch + 1;

    let applied = applier
        .try_apply_checkpoint_results(&checkpoint_data(other_epoch, vec![checkpoint_tx]))
        .expect("skipping is not an error");

    assert!(
        !applied,
        "a checkpoint from another epoch must not be applied"
    );
    assert!(
        !is_executed(&authority, &tx_digest),
        "nothing may be written for a checkpoint that was not applied"
    );
}

/// Verification runs before any write, so a checkpoint carrying a tampered
/// object leaves the store untouched and the executor can still produce the
/// results itself.
#[tokio::test]
async fn rejects_tampered_data_without_writing() {
    let (authority, applier, mut checkpoint_tx) = applier_with_transfer().await;
    let tx_digest = *checkpoint_tx.effects.transaction_digest();
    let epoch = authority.epoch_store_for_testing().epoch();
    let mut tampered = checkpoint_tx.output_objects[0].as_inner().clone();
    tampered.storage_rebate += 1;
    checkpoint_tx.output_objects[0] = tampered.into();

    let error = applier
        .try_apply_checkpoint_results(&checkpoint_data(epoch, vec![checkpoint_tx]))
        .expect_err("a tampered object must be rejected");

    assert!(
        format!("{error}").contains("object mismatch"),
        "the error must name the failure, got: {error}"
    );
    assert!(
        !is_executed(&authority, &tx_digest),
        "a rejected checkpoint must not have written anything"
    );
}

/// Waiting for an epoch the node has already reached must return at once, so
/// archive sync is never paused for checkpoints it can apply straight away.
/// The waiting path itself needs a reconfiguration and is covered end to end.
#[tokio::test]
async fn wait_for_epoch_returns_immediately_when_already_current() {
    let (authority, applier, _) = applier_with_transfer().await;
    let current = authority.epoch_store_for_testing().epoch();

    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        applier.wait_for_epoch(current),
    )
    .await
    .expect("must not pause for an epoch the node is already in");
}
