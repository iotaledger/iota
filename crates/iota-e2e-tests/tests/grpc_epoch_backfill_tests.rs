// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::Path, sync::Arc};

use iota_core::checkpoints::CheckpointStore;
use iota_macros::sim_test;
use iota_snapshot::{EpochInfo, EpochInfoV1};
use iota_types::{
    committee::EpochId,
    digests::{ChainIdentifier, TransactionDigest},
    effects::{TransactionEffects, TransactionEffectsExtForTesting, TransactionEvents},
    messages_checkpoint::CheckpointContents,
    storage::EpochInfoV1Entry,
};
use test_cluster::TestClusterBuilder;

/// Wait until the fullnode has executed every closed epoch's closing
/// checkpoint (its executed open epoch reached `open_epoch`). The tests below
/// read closed-epoch data from the live stores; without this, a closing
/// checkpoint executed mid-test shifts the gap computation under the
/// assertions (the cluster keeps running while the test works).
async fn wait_until_executed_open_epoch(checkpoint_store: &CheckpointStore, open_epoch: u64) {
    loop {
        let executed_open_epoch = checkpoint_store
            .get_highest_executed_checkpoint()
            .unwrap()
            .map(|checkpoint| {
                if checkpoint.is_last_checkpoint_of_epoch() {
                    checkpoint.epoch + 1
                } else {
                    checkpoint.epoch
                }
            });
        if executed_open_epoch >= Some(open_epoch) {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

/// Assert a node's live `epoch_info` chain is complete: every closed epoch
/// finalized, the watermark covering them, no gap, and the open epoch seeded.
async fn assert_epoch_info_chain_complete(checkpoint_store: &CheckpointStore, current_epoch: u64) {
    assert!(current_epoch >= 2, "need at least two closed epochs");
    wait_until_executed_open_epoch(checkpoint_store, current_epoch).await;

    for epoch in 0..current_epoch {
        let row = checkpoint_store
            .get_epoch_info(epoch)
            .unwrap()
            .unwrap_or_else(|| panic!("closed epoch {epoch} must have a row"));
        assert!(
            row.is_finalized(),
            "closed epoch {epoch}'s row must be finalized by its boundary"
        );
    }
    assert_eq!(
        checkpoint_store.highest_indexed_epoch().unwrap(),
        Some(current_epoch - 1),
        "the completeness watermark must cover every closed epoch"
    );
    assert_eq!(
        checkpoint_store.epoch_info_gap().unwrap(),
        None,
        "live population must leave no gap"
    );
    assert!(
        checkpoint_store
            .get_epoch_info(current_epoch)
            .unwrap()
            .is_some(),
        "the open epoch's row must be seeded, pending finalization at its boundary"
    );
}

/// Every node — the fullnode and each validator — populates its `epoch_info`
/// chain from its own executed boundaries, leaving a complete chain.
#[sim_test]
async fn epoch_info_chain_is_populated_live() {
    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(600_000)
        .disable_fullnode_pruning()
        .build()
        .await;
    test_cluster.force_new_epoch().await;
    test_cluster.force_new_epoch().await;

    // Read each node's own checkpoint store and current epoch; validators may
    // sit at a slightly different epoch than the fullnode.
    let (fullnode_store, fullnode_epoch) = test_cluster.fullnode_handle.iota_node.with(|node| {
        let state = node.state();
        (
            state.get_checkpoint_store().clone(),
            state.current_epoch_for_testing(),
        )
    });
    assert_epoch_info_chain_complete(&fullnode_store, fullnode_epoch).await;

    for handle in test_cluster.all_validator_handles() {
        let (validator_store, validator_epoch) = handle.with(|node| {
            let state = node.state();
            (
                state.get_checkpoint_store().clone(),
                state.current_epoch_for_testing(),
            )
        });
        assert_epoch_info_chain_complete(&validator_store, validator_epoch).await;
    }
}

/// EPOCH_INFO that matches the manifest `chain_id` and chain-verifies against
/// the genesis committee seeds every closed epoch into a target store; a
/// chain-id mismatch or a non-genesis trust root yields no verified witness, so
/// nothing is written.
#[sim_test]
async fn epoch_info_backfill_verifies_chain_before_seeding() {
    // Long epoch duration so the epoch only advances when forced, keeping the
    // closed-epoch set stable across the assertions. Pruning disabled so the
    // live store keeps every closed epoch's finalized row.
    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(600_000)
        .disable_fullnode_pruning()
        .build()
        .await;
    test_cluster.force_new_epoch().await;
    test_cluster.force_new_epoch().await;

    let state = test_cluster
        .fullnode_handle
        .iota_node
        .with(|node| node.state());
    let checkpoint_store = state.get_checkpoint_store().clone();
    // Closed epochs are `[0, current)`; EPOCH_INFO covers exactly those.
    let current_epoch = state.current_epoch_for_testing();
    assert!(current_epoch >= 2, "need at least two closed epochs");
    wait_until_executed_open_epoch(&checkpoint_store, current_epoch).await;

    // This node's chain id — what a same-chain snapshot's manifest carries.
    let expected_chain_id = state.get_chain_identifier();

    // The trust roots for the committee-chain walk and epoch 0's start state.
    let genesis = &test_cluster.swarm.config().genesis;
    let genesis_committee = genesis.committee().expect("genesis committee");
    let genesis_system_state = genesis.iota_system_object();

    // The node's live `epoch_info` chain supplies the real proof bundles.
    let source = &*checkpoint_store;

    // SUCCESS: a matching chain_id + committee chain seeds every closed epoch
    // into a fresh CheckpointStore.
    let ok_tmp = tempfile::tempdir().unwrap();
    let ok_store = CheckpointStore::new(&ok_tmp.path().join("checkpoints"));
    let verified = iota_snapshot::verify_epoch_info_chain(
        real_epoch_info(source, current_epoch),
        genesis_committee.clone(),
        genesis_system_state.clone(),
        expected_chain_id,
        expected_chain_id,
    )
    .expect("a same-chain snapshot must verify");
    // The committee chain spans `[0, snapshot_epoch + 1]`: the genesis committee
    // plus the one each closed epoch hands forward. The formal-snapshot seed
    // path (`download_formal_snapshot`) zips these against the entries, so a
    // wrong length would silently truncate it.
    let committee_epochs: Vec<_> = verified.committees().iter().map(|c| c.epoch).collect();
    assert_eq!(
        committee_epochs,
        (0..=current_epoch).collect::<Vec<_>>(),
        "committees() must carry one committee per epoch in [0, current_epoch]"
    );
    verified
        .restore_epoch_info(&*ok_store)
        .await
        .expect("a verified snapshot must seed");
    for epoch in 0..current_epoch {
        assert!(
            ok_store.get_epoch_info(epoch).unwrap().is_some(),
            "epoch {epoch} must be seeded after a verified backfill"
        );
    }

    // Re-seeding an already-covered store is a no-op: the watermark spans every
    // epoch, so the write skips the whole prefix.
    let watermark = ok_store.highest_indexed_epoch().unwrap();
    assert_eq!(watermark, Some(current_epoch - 1));
    iota_snapshot::verify_epoch_info_chain(
        real_epoch_info(source, current_epoch),
        genesis_committee.clone(),
        genesis_system_state.clone(),
        expected_chain_id,
        expected_chain_id,
    )
    .expect("re-verification must succeed")
    .restore_epoch_info(&*ok_store)
    .await
    .expect("re-seeding a covered store must succeed as a no-op");
    assert_eq!(
        ok_store.highest_indexed_epoch().unwrap(),
        watermark,
        "re-seeding must leave the watermark unchanged"
    );

    // FAILURE: a different chain_id never yields a verified witness, so
    // NOTHING can be written.
    let err = iota_snapshot::verify_epoch_info_chain(
        real_epoch_info(source, current_epoch),
        genesis_committee.clone(),
        genesis_system_state.clone(),
        ChainIdentifier::default(),
        expected_chain_id,
    )
    .expect_err("a wrong-network snapshot must be rejected");
    assert!(
        err.to_string().contains("chain_id"),
        "expected a chain_id mismatch error, got: {err}"
    );

    // FAILURE: a genesis system state whose embedded committee doesn't match
    // the genesis committee is caught by the start-state committee-match check.
    // A later epoch's real start state stands in: same validators, but its
    // committee is stamped with the wrong epoch.
    let later_start_state = source
        .get_epoch_info(1)
        .unwrap()
        .expect("epoch 1 is finalized")
        .system_state;
    let err = iota_snapshot::verify_epoch_info_chain(
        real_epoch_info(source, current_epoch),
        genesis_committee.clone(),
        later_start_state,
        expected_chain_id,
        expected_chain_id,
    )
    .expect_err("a genesis system state with a mismatched committee must be rejected");
    assert!(
        err.to_string()
            .contains("does not match the certified committee"),
        "expected a start-state committee-match error, got: {err}"
    );

    // FAILURE: a wrong trust root (the current committee instead of the
    // genesis one, after two reconfigurations) fails the chain walk.
    let err = iota_snapshot::verify_epoch_info_chain(
        real_epoch_info(source, current_epoch),
        state.clone_committee_for_testing(),
        genesis_system_state,
        expected_chain_id,
        expected_chain_id,
    )
    .expect_err("a non-genesis trust root must be rejected");
    assert!(
        err.to_string().contains("genesis committee"),
        "expected a trust-root error, got: {err}"
    );
}

/// Corrupting any one link of an entry's proof bundle (contents, effects,
/// events, start-state objects, or the unsigned `start_checkpoint`) is rejected
/// even though the committee chain still verifies. The safe-mode boundary is
/// covered separately by `epoch_info_verifies_safe_mode_boundary`.
#[sim_test]
async fn epoch_info_proof_bundle_tampering_is_rejected() {
    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(600_000)
        .disable_fullnode_pruning()
        .build()
        .await;
    test_cluster.force_new_epoch().await;
    test_cluster.force_new_epoch().await;

    let state = test_cluster
        .fullnode_handle
        .iota_node
        .with(|node| node.state());
    let checkpoint_store = state.get_checkpoint_store().clone();
    let current_epoch = state.current_epoch_for_testing();
    assert!(current_epoch >= 2, "need at least two closed epochs");
    wait_until_executed_open_epoch(&checkpoint_store, current_epoch).await;

    let chain_id = state.get_chain_identifier();
    let genesis = &test_cluster.swarm.config().genesis;
    let genesis_committee = genesis.committee().expect("genesis committee");
    let genesis_system_state = genesis.iota_system_object();

    let source = &*checkpoint_store;

    // Sanity: the untampered bundle verifies, so each rejection below is the
    // tamper's doing, not a fixture defect.
    iota_snapshot::verify_epoch_info_chain(
        real_epoch_info(source, current_epoch),
        genesis_committee.clone(),
        genesis_system_state.clone(),
        chain_id,
        chain_id,
    )
    .expect("the untampered EPOCH_INFO must verify");

    // The last closed epoch's boundary is a normal `AdvanceEpoch` (events
    // present), so every link is non-trivially populated. Each case also
    // asserts the distinctive error fragment of the check it should trip, so a
    // tamper that happens to fail some *other* index-`idx` check can't pass.
    let idx = (current_epoch - 1) as usize;
    #[allow(clippy::type_complexity)]
    let cases: Vec<(&str, &str, Box<dyn Fn(&mut EpochInfoV1Entry)>)> = vec![
        (
            "contents",
            "does not hash to the signed content_digest",
            Box::new(|entry| {
                entry.last_checkpoint_contents =
                    CheckpointContents::new_with_digests_only_for_tests(std::iter::empty());
            }),
        ),
        (
            "effects",
            "digest pair does not match the closing checkpoint's last transaction",
            Box::new(|entry| {
                entry.end_of_epoch_tx_effects =
                    TransactionEffects::new_empty_v1_for_testing(TransactionDigest::ZERO);
            }),
        ),
        (
            "events",
            "does not hash to the effects' events_digest",
            Box::new(|entry| {
                entry.end_of_epoch_tx_events = TransactionEvents::default();
            }),
        ),
        (
            "start-state objects",
            "do not include the system-state object 0x5",
            Box::new(|entry| {
                entry.next_epoch_start_system_state_objects = Vec::new();
            }),
        ),
        (
            "summary",
            "failed verification",
            Box::new(|entry| {
                // Mutate a signed field while keeping the quorum signature: the
                // certificate carries a real signature but no longer verifies.
                let mut data = entry.last_checkpoint_summary.data().clone();
                data.timestamp_ms = data.timestamp_ms.wrapping_add(1);
                entry.last_checkpoint_summary =
                    iota_types::message_envelope::Envelope::new_from_data_and_sig(
                        data,
                        entry.last_checkpoint_summary.auth_sig().clone(),
                    );
            }),
        ),
    ];

    for (label, expected_fragment, mutate) in cases {
        let EpochInfo::V1(mut info) = real_epoch_info(source, current_epoch);
        mutate(&mut info.entries[idx]);
        let err = iota_snapshot::verify_epoch_info_chain(
            EpochInfo::V1(info),
            genesis_committee.clone(),
            genesis_system_state.clone(),
            chain_id,
            chain_id,
        )
        .unwrap_err();
        let err = err.to_string();
        assert!(
            err.contains(&format!("epoch {idx}")) && err.contains(expected_fragment),
            "tampering with {label} must be rejected at epoch {idx} with \
             {expected_fragment:?}, got: {err}"
        );
    }
}

/// A safe-mode boundary (`advance_epoch_safe_mode`, which emits no events) must
/// chain-verify: its effects carry no `events_digest`, so the verifier requires
/// the entry's events to be empty rather than hashing them. Forced via the
/// `advance_epoch` failure injection, so this is `msim`-only.
#[cfg(msim)]
#[sim_test]
async fn epoch_info_verifies_safe_mode_boundary() {
    use iota_types::{
        effects::TransactionEffectsAPI, iota_system_state::advance_epoch_result_injection,
    };

    // Fail the advance into epoch 2, so epoch 1's closing boundary runs
    // `advance_epoch_safe_mode`; epochs 0 and 2 close normally (events present).
    advance_epoch_result_injection::set_override(Some((2, 3)));

    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(600_000)
        .disable_fullnode_pruning()
        .build()
        .await;
    // Suppress the node's "never safe mode" debug assertion for the injected
    // epoch (one-way: leaving it set never forces safe mode on normal epochs).
    test_cluster.set_safe_mode_expected(true);
    // 0->1 normal, 1->2 safe mode, 2->3 normal.
    test_cluster.force_new_epoch().await;
    test_cluster.force_new_epoch().await;
    test_cluster.force_new_epoch().await;

    let state = test_cluster
        .fullnode_handle
        .iota_node
        .with(|node| node.state());
    let checkpoint_store = state.get_checkpoint_store().clone();
    let current_epoch = state.current_epoch_for_testing();
    assert!(
        current_epoch >= 3,
        "need the safe-mode epoch plus one normal epoch closed after it"
    );
    wait_until_executed_open_epoch(&checkpoint_store, current_epoch).await;

    let chain_id = state.get_chain_identifier();
    let genesis = &test_cluster.swarm.config().genesis;
    let genesis_committee = genesis.committee().expect("genesis committee");
    let genesis_system_state = genesis.iota_system_object();

    let source = &*checkpoint_store;
    let epoch_info = real_epoch_info(source, current_epoch);

    // Confirm the chain genuinely contains a safe-mode boundary — otherwise the
    // verify below would pass on all-normal data. Exactly one entry's
    // epoch-change effects carry no `events_digest`, and its events are empty.
    let EpochInfo::V1(info) = &epoch_info;
    let safe_mode: Vec<_> = info
        .entries
        .iter()
        .filter(|entry| entry.end_of_epoch_tx_effects.events_digest().is_none())
        .collect();
    assert_eq!(
        safe_mode.len(),
        1,
        "expected exactly one safe-mode boundary in [0, {current_epoch})"
    );
    assert!(
        safe_mode[0].end_of_epoch_tx_events.is_empty(),
        "a safe-mode boundary must carry no events"
    );

    // REJECT path: the safe-mode boundary's effects carry no `events_digest`,
    // so the verifier requires its events to be empty. Giving it a non-empty
    // events list (borrowed from a normal boundary in the same chain) must be
    // rejected at that entry.
    let safe_mode_epoch = info
        .entries
        .iter()
        .position(|entry| entry.end_of_epoch_tx_effects.events_digest().is_none())
        .expect("a safe-mode boundary exists");
    let normal_events = info
        .entries
        .iter()
        .find(|entry| !entry.end_of_epoch_tx_events.is_empty())
        .expect("a normal boundary with events exists")
        .end_of_epoch_tx_events
        .clone();
    let EpochInfo::V1(mut tampered) = real_epoch_info(source, current_epoch);
    tampered.entries[safe_mode_epoch].end_of_epoch_tx_events = normal_events;
    let err = iota_snapshot::verify_epoch_info_chain(
        EpochInfo::V1(tampered),
        genesis_committee.clone(),
        genesis_system_state.clone(),
        chain_id,
        chain_id,
    )
    .unwrap_err()
    .to_string();
    assert!(
        err.contains(&format!("epoch {safe_mode_epoch}")) && err.contains("carry no events_digest"),
        "a safe-mode boundary with non-empty events must be rejected, got: {err}"
    );

    // The whole chain, safe-mode boundary included, must verify and seed.
    let tmp = tempfile::tempdir().unwrap();
    let target = CheckpointStore::new(&tmp.path().join("checkpoints"));
    iota_snapshot::verify_epoch_info_chain(
        epoch_info,
        genesis_committee,
        genesis_system_state,
        chain_id,
        chain_id,
    )
    .expect("a chain with a safe-mode boundary must verify")
    .restore_epoch_info(&*target)
    .await
    .expect("a verified safe-mode chain must seed");
    for epoch in 0..current_epoch {
        assert!(
            target.get_epoch_info(epoch).unwrap().is_some(),
            "epoch {epoch} must be seeded"
        );
    }
}

/// When a snapshot backfill seeds a prefix that ends below the locally executed
/// epochs (the published snapshot lags by more than one epoch),
/// `index_missing_epochs_locally` closes the residual gap from the missing
/// epochs' own closing checkpoints — and creates the open epoch's row along the
/// way.
#[sim_test]
async fn missing_epochs_above_snapshot_prefix_are_indexed_locally() {
    // Pruning disabled so the closing checkpoints' data is still available
    // locally — the precondition for the local replay.
    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(600_000)
        .disable_fullnode_pruning()
        .build()
        .await;
    test_cluster.force_new_epoch().await;
    test_cluster.force_new_epoch().await;

    let state = test_cluster
        .fullnode_handle
        .iota_node
        .with(|node| node.state());
    let authority_store = state.database_for_testing();
    let node_checkpoint_store = state.get_checkpoint_store().clone();
    let current_epoch = state.current_epoch_for_testing();
    assert!(current_epoch >= 2, "need at least two closed epochs");
    wait_until_executed_open_epoch(&node_checkpoint_store, current_epoch).await;

    let expected_chain_id = state.get_chain_identifier();
    let genesis = &test_cluster.swarm.config().genesis;
    let genesis_committee = genesis.committee().expect("genesis committee");
    let genesis_system_state = genesis.iota_system_object();

    // Staging store with only the closing checkpoints (what the replay reads)
    // and no `epoch_info` rows — the state a lagging-snapshot backfill leaves.
    let staging_tmp = tempfile::tempdir().unwrap();
    let staged =
        stage_closing_checkpoints(staging_tmp.path(), &node_checkpoint_store, current_epoch);

    // Mimic a backfill from a lagging snapshot: seed only epoch 0, leaving the
    // later closed epochs missing.
    iota_snapshot::verify_epoch_info_chain(
        real_epoch_info(&node_checkpoint_store, 1),
        genesis_committee,
        genesis_system_state,
        expected_chain_id,
        expected_chain_id,
    )
    .expect("the lagging snapshot's prefix must verify")
    .restore_epoch_info(&*staged)
    .await
    .expect("seeding the lagging snapshot's prefix must succeed");
    assert_eq!(staged.highest_indexed_epoch().unwrap(), Some(0));
    assert!(
        staged.epoch_info_gap().unwrap().is_some(),
        "the lagging prefix must leave a gap"
    );

    staged
        .index_missing_epochs_locally(&authority_store)
        .unwrap();

    assert_eq!(
        staged.epoch_info_gap().unwrap(),
        None,
        "the local replay must close the residual gap"
    );
    // The last replayed closing checkpoint also creates the open epoch's row.
    assert!(
        staged.get_epoch_info(current_epoch).unwrap().is_some(),
        "the local replay must create the open epoch's row"
    );
}

/// Build a real `EpochInfo` for closed epochs `[0, current_epoch)` from each
/// `epoch_info` row's close-of-epoch proof. Panics unless `source` has a
/// finalized row for every closed epoch.
fn real_epoch_info(source: &CheckpointStore, current_epoch: EpochId) -> EpochInfo {
    let entries = (0..current_epoch)
        .map(|epoch| {
            let row = source
                .get_epoch_info(epoch)
                .unwrap()
                .unwrap_or_else(|| panic!("missing epoch_info row for closed epoch {epoch}"));
            row.epoch_close_proof.unwrap_or_else(|| {
                panic!("epoch_info row for closed epoch {epoch} is not finalized")
            })
        })
        .collect();
    EpochInfo::V1(EpochInfoV1 { entries })
}

/// Copy the closing checkpoints of closed epochs `[0, current_epoch)` from
/// `node` into a fresh store, with its highest-executed watermark set to the
/// last one so its first open epoch is `current_epoch`. Writes no `epoch_info`
/// rows: the caller seeds a partial prefix and runs the local replay over this.
fn stage_closing_checkpoints(
    dir: &Path,
    node: &CheckpointStore,
    current_epoch: EpochId,
) -> Arc<CheckpointStore> {
    let staged = CheckpointStore::new(&dir.join("checkpoints"));
    let mut last_closing = None;
    for epoch in 0..current_epoch {
        let closing = node
            .get_epoch_last_checkpoint(epoch)
            .unwrap()
            .unwrap_or_else(|| panic!("node missing closing checkpoint for epoch {epoch}"));
        let contents = node
            .get_checkpoint_contents(&closing.content_digest)
            .unwrap()
            .expect("node missing closing checkpoint contents");
        staged.insert_checkpoint_contents(contents).unwrap();
        // Writes the certified checkpoint and (since closing checkpoints carry
        // `next_epoch_committee`) the `epoch_last_checkpoint_map` entry the
        // replay reads.
        staged.insert_verified_checkpoint(&closing).unwrap();
        last_closing = Some(closing);
    }
    let last_closing = last_closing.expect("current_epoch >= 1");
    staged
        .update_highest_executed_checkpoint(&last_closing)
        .unwrap();
    staged
}
