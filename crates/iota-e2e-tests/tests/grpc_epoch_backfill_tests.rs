// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_core::{checkpoints::CheckpointStore, grpc_indexes::GrpcIndexesStore};
use iota_macros::sim_test;
use iota_snapshot::{EpochInfo, EpochInfoV1};
use iota_types::{
    committee::EpochId,
    digests::{ChainIdentifier, TransactionDigest},
    effects::{TransactionEffects, TransactionEffectsExtForTesting, TransactionEvents},
    iota_system_state::IotaSystemState,
    messages_checkpoint::CheckpointContents,
    storage::{EpochInfoV1Entry, EpochInfoV2},
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

/// A formal-snapshot restore (unless `--skip-grpc-indexes` is passed) writes
/// the store's contents and then `finalize_restore`s it (`Watermark::Indexed`
/// + `meta`).
/// `GrpcIndexesStore::new` must open such a store in place rather than wipe
/// and re-index, while an *unfinalized* store (e.g. a restore that crashed
/// mid-way) must still be wiped.
///
/// Uses a sentinel row at an epoch far beyond the node's range, which `init`
/// can never recreate: its survival across `new` proves the wipe was skipped.
#[sim_test]
async fn finalized_restore_survives_grpc_indexes_store_init() {
    let test_cluster = TestClusterBuilder::new().build().await;

    let state = test_cluster
        .fullnode_handle
        .iota_node
        .with(|node| node.state());
    let authority_store = state.database_for_testing();
    let checkpoint_store = state.get_checkpoint_store().clone();

    // An epoch the node will never index, so `init` cannot recreate its row.
    let sentinel_epoch = state.current_epoch_for_testing() + 1_000;

    // Mimic a completed restore: write a row, then finalize. The restore tool
    // finalizes a *stopped* staging DB whose executed watermark can't move
    // afterwards; the cluster here keeps executing checkpoints, so finalize
    // far ahead to keep the store's watermark current when `new` runs.
    let finalized_tmp = tempfile::tempdir().unwrap();
    let finalized_path = finalized_tmp.path().to_path_buf();
    {
        let grpc = GrpcIndexesStore::new_without_init(finalized_path.clone());
        grpc.insert_epoch_info(vec![restored_sentinel_row(sentinel_epoch)])
            .unwrap();
        grpc.finalize_restore(u64::MAX).unwrap();
    }
    let grpc =
        GrpcIndexesStore::new(finalized_path, authority_store.clone(), &checkpoint_store).await;
    assert!(
        grpc.get_epoch_info(sentinel_epoch).unwrap().is_some(),
        "finalized restore was discarded — the store was wiped and re-initialized"
    );

    // Without the finalize (crashed restore), `new` must wipe and re-init.
    let unfinalized_tmp = tempfile::tempdir().unwrap();
    let unfinalized_path = unfinalized_tmp.path().to_path_buf();
    {
        let grpc = GrpcIndexesStore::new_without_init(unfinalized_path.clone());
        grpc.insert_epoch_info(vec![restored_sentinel_row(sentinel_epoch)])
            .unwrap();
    }
    let grpc = GrpcIndexesStore::new(unfinalized_path, authority_store, &checkpoint_store).await;
    assert!(
        grpc.get_epoch_info(sentinel_epoch).unwrap().is_none(),
        "an unfinalized restore must be wiped and re-initialized"
    );
}

/// EPOCH_INFO that matches the manifest `chain_id` and chain-verifies against
/// the genesis committee seeds every closed epoch; a chain-id mismatch or a
/// non-genesis trust root never yields the verified witness required to
/// write anything.
#[sim_test]
async fn epoch_info_backfill_verifies_chain_before_seeding() {
    // Long epoch duration so the epoch only advances when forced, keeping the
    // closed-epoch set stable across the assertions. Pruning disabled so the
    // local source store can rebuild every closed epoch's proof bundle.
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

    // A locally-indexed store gives every closed epoch its real proof bundle,
    // the way the snapshot writer reads them off `epochs_v2`.
    let source_tmp = tempfile::tempdir().unwrap();
    let source = GrpcIndexesStore::new(
        source_tmp.path().to_path_buf(),
        authority_store,
        &checkpoint_store,
    )
    .await;

    // SUCCESS: a matching chain_id + committee chain seeds every closed epoch
    // into a fresh store.
    let ok_tmp = tempfile::tempdir().unwrap();
    let ok_grpc = GrpcIndexesStore::new_without_init(ok_tmp.path().to_path_buf());
    let verified = iota_snapshot::verify_epoch_info_chain(
        real_epoch_info(&source, current_epoch),
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
        .restore_epoch_info(&ok_grpc)
        .await
        .expect("a verified snapshot must seed");
    for epoch in 0..current_epoch {
        assert!(
            ok_grpc.get_epoch_info(epoch).unwrap().is_some(),
            "epoch {epoch} must be seeded after a verified backfill"
        );
    }

    // Re-seeding an already-covered store is a no-op: the watermark spans every
    // epoch, so the write skips the whole prefix.
    let watermark = ok_grpc.highest_indexed_epoch().unwrap();
    assert_eq!(watermark, Some(current_epoch - 1));
    iota_snapshot::verify_epoch_info_chain(
        real_epoch_info(&source, current_epoch),
        genesis_committee.clone(),
        genesis_system_state.clone(),
        expected_chain_id,
        expected_chain_id,
    )
    .expect("re-verification must succeed")
    .restore_epoch_info(&ok_grpc)
    .await
    .expect("re-seeding a covered store must succeed as a no-op");
    assert_eq!(
        ok_grpc.highest_indexed_epoch().unwrap(),
        watermark,
        "re-seeding must leave the watermark unchanged"
    );

    // FAILURE: a different chain_id never yields a verified witness, so
    // NOTHING can be written.
    let err = iota_snapshot::verify_epoch_info_chain(
        real_epoch_info(&source, current_epoch),
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
        .expect("epoch 1 is indexed")
        .system_state;
    let err = iota_snapshot::verify_epoch_info_chain(
        real_epoch_info(&source, current_epoch),
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
        real_epoch_info(&source, current_epoch),
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
    let authority_store = state.database_for_testing();
    let checkpoint_store = state.get_checkpoint_store().clone();
    let current_epoch = state.current_epoch_for_testing();
    assert!(current_epoch >= 2, "need at least two closed epochs");
    wait_until_executed_open_epoch(&checkpoint_store, current_epoch).await;

    let chain_id = state.get_chain_identifier();
    let genesis = &test_cluster.swarm.config().genesis;
    let genesis_committee = genesis.committee().expect("genesis committee");
    let genesis_system_state = genesis.iota_system_object();

    let source_tmp = tempfile::tempdir().unwrap();
    let source = GrpcIndexesStore::new(
        source_tmp.path().to_path_buf(),
        authority_store,
        &checkpoint_store,
    )
    .await;

    // Sanity: the untampered bundle verifies, so each rejection below is the
    // tamper's doing, not a fixture defect.
    iota_snapshot::verify_epoch_info_chain(
        real_epoch_info(&source, current_epoch),
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
        let EpochInfo::V1(mut info) = real_epoch_info(&source, current_epoch);
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
    let authority_store = state.database_for_testing();
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

    let source_tmp = tempfile::tempdir().unwrap();
    let source = GrpcIndexesStore::new(
        source_tmp.path().to_path_buf(),
        authority_store,
        &checkpoint_store,
    )
    .await;
    let epoch_info = real_epoch_info(&source, current_epoch);

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
    let EpochInfo::V1(mut tampered) = real_epoch_info(&source, current_epoch);
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
    let grpc = GrpcIndexesStore::new_without_init(tmp.path().to_path_buf());
    iota_snapshot::verify_epoch_info_chain(
        epoch_info,
        genesis_committee,
        genesis_system_state,
        chain_id,
        chain_id,
    )
    .expect("a chain with a safe-mode boundary must verify")
    .restore_epoch_info(&grpc)
    .await
    .expect("a verified safe-mode chain must seed");
    for epoch in 0..current_epoch {
        assert!(
            grpc.get_epoch_info(epoch).unwrap().is_some(),
            "epoch {epoch} must be seeded"
        );
    }
}

/// `epochs_v2_gap` (the startup guard's check) reports no gap for a
/// full-history index but a gap for an empty index past genesis.
#[sim_test]
async fn epochs_v2_gap_detects_incomplete_index() {
    // Pruning disabled so the fullnode keeps full history back to genesis —
    // the case where `init` + local replay can complete `epochs_v2` with no gap.
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
    let checkpoint_store = state.get_checkpoint_store().clone();
    let current_epoch = state.current_epoch_for_testing();
    assert!(current_epoch >= 2);
    wait_until_executed_open_epoch(&checkpoint_store, current_epoch).await;

    // Full-history `new()` indexes `[0, current)` from local data → no gap, so a
    // gRPC node here would NOT panic.
    let ok_tmp = tempfile::tempdir().unwrap();
    let complete = GrpcIndexesStore::new(
        ok_tmp.path().to_path_buf(),
        authority_store,
        &checkpoint_store,
    )
    .await;
    assert!(
        complete.epochs_v2_gap(&checkpoint_store).unwrap().is_none(),
        "a full-history index must be complete (no startup panic)"
    );

    // An un-initialized (empty) index, with the node already past genesis,
    // reports a gap → a gRPC node here would require state_snapshot_read_config.
    let gap_tmp = tempfile::tempdir().unwrap();
    let empty = GrpcIndexesStore::new_without_init(gap_tmp.path().to_path_buf());
    assert!(
        empty.epochs_v2_gap(&checkpoint_store).unwrap().is_some(),
        "an empty index past genesis must report a gap"
    );
}

/// When the snapshot backfill seeds a prefix that ends below the locally
/// executed epochs (the published snapshot lags by more than one epoch),
/// `index_missing_epochs_locally` closes the residual gap from the missing
/// epochs' own closing checkpoints — and creates the open epoch's row along
/// the way.
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
    let checkpoint_store = state.get_checkpoint_store().clone();
    let current_epoch = state.current_epoch_for_testing();
    assert!(current_epoch >= 2, "need at least two closed epochs");
    wait_until_executed_open_epoch(&checkpoint_store, current_epoch).await;

    let expected_chain_id = state.get_chain_identifier();
    let genesis = &test_cluster.swarm.config().genesis;
    let genesis_committee = genesis.committee().expect("genesis committee");
    let genesis_system_state = genesis.iota_system_object();

    // A locally-indexed store gives epoch 0 its real proof bundle.
    let source_tmp = tempfile::tempdir().unwrap();
    let source = GrpcIndexesStore::new(
        source_tmp.path().to_path_buf(),
        authority_store.clone(),
        &checkpoint_store,
    )
    .await;

    // Mimic a backfill from a lagging snapshot: seed only epoch 0, leaving
    // the later closed epochs missing.
    let tmp = tempfile::tempdir().unwrap();
    let grpc = GrpcIndexesStore::new_without_init(tmp.path().to_path_buf());
    iota_snapshot::verify_epoch_info_chain(
        real_epoch_info(&source, 1),
        genesis_committee,
        genesis_system_state,
        expected_chain_id,
        expected_chain_id,
    )
    .expect("the lagging snapshot's prefix must verify")
    .restore_epoch_info(&grpc)
    .await
    .expect("seeding the lagging snapshot's prefix must succeed");
    assert_eq!(grpc.highest_indexed_epoch().unwrap(), Some(0));
    assert!(
        grpc.epochs_v2_gap(&checkpoint_store).unwrap().is_some(),
        "the lagging prefix must leave a gap"
    );

    grpc.index_missing_epochs_locally(&authority_store, &checkpoint_store)
        .unwrap();

    assert_eq!(
        grpc.epochs_v2_gap(&checkpoint_store).unwrap(),
        None,
        "the local replay must close the residual gap"
    );
    // The last replayed closing checkpoint also creates the open epoch's row.
    let open_epoch = grpc.highest_indexed_epoch().unwrap().unwrap() + 1;
    assert!(grpc.get_epoch_info(open_epoch).unwrap().is_some());
}

/// Build a real `EpochInfo` for closed epochs `[0, current_epoch)` from a
/// locally-indexed `epochs_v2` store, projecting each row into its entry via
/// the same `EpochInfoV1Entry::try_from` the snapshot writer uses. `source`
/// must have fully-populated rows for every closed epoch (e.g. a freshly
/// `GrpcIndexesStore::new`-indexed store over unpruned history).
fn real_epoch_info(source: &GrpcIndexesStore, current_epoch: EpochId) -> EpochInfo {
    let entries = (0..current_epoch)
        .map(|epoch| {
            let row = source
                .get_epoch_info(epoch)
                .unwrap()
                .unwrap_or_else(|| panic!("missing epochs_v2 row for closed epoch {epoch}"));
            row.epoch_close_proof.unwrap_or_else(|| {
                panic!("epochs_v2 row for closed epoch {epoch} is not finalized")
            })
        })
        .collect();
    EpochInfo::V1(EpochInfoV1 { entries })
}

/// A minimal `epochs_v2` row standing in for one written by a snapshot restore.
/// Only `epoch` and a decodable `system_state` matter here — the close-of-epoch
/// entry is left `None` since the test asserts row survival, not completeness.
fn restored_sentinel_row(epoch: u64) -> EpochInfoV2 {
    EpochInfoV2 {
        epoch,
        start_checkpoint: 0,
        start_timestamp_ms: 0,
        system_state: IotaSystemState::for_testing(epoch, 1),
        epoch_close_proof: None,
    }
}
