// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_core::{checkpoints::CheckpointStore, grpc_indexes::GrpcIndexesStore};
use iota_macros::sim_test;
use iota_snapshot::{EpochInfo, EpochInfoV1, EpochInfoV1Entry};
use iota_types::{
    committee::EpochId, digests::ChainIdentifier, iota_system_state::IotaSystemState,
    storage::EpochInfoV2,
};
use test_cluster::TestClusterBuilder;

/// A snapshot restore writes `epochs_v2` rows before the store is initialized
/// (no `meta`). `GrpcIndexesStore::new` must detect that and initialize in
/// place (skipping `safe_drop_db`) so the restored rows are kept.
///
/// Uses a sentinel row at an epoch far beyond the node's range, which `init`
/// can never recreate: its survival across `new` proves the drop was skipped.
#[sim_test]
async fn restored_epoch_rows_survive_grpc_indexes_store_init() {
    let test_cluster = TestClusterBuilder::new().build().await;

    let state = test_cluster
        .fullnode_handle
        .iota_node
        .with(|node| node.state());
    let authority_store = state.database_for_testing();
    let checkpoint_store = state.get_checkpoint_store().clone();

    // An epoch the node will never index, so `init` cannot recreate its row.
    let sentinel_epoch = state.current_epoch_for_testing() + 1_000;

    // Mimic a restore: open an uninitialized store (no `meta`) and write a row,
    // leaving the rows-without-`meta` state `has_restored_epoch_info` detects.
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().to_path_buf();
    {
        let grpc = GrpcIndexesStore::new_without_init(path.clone());
        grpc.insert_epoch_info(vec![restored_sentinel_row(sentinel_epoch)])
            .unwrap();
        assert!(grpc.get_epoch_info(sentinel_epoch).unwrap().is_some());
    }

    // `new` must initialize in place and preserve the restored row.
    let grpc = GrpcIndexesStore::new(path, authority_store, &checkpoint_store).await;
    assert!(
        grpc.get_epoch_info(sentinel_epoch).unwrap().is_some(),
        "restored row was discarded — `safe_drop_db` ran instead of in-place init"
    );
}

/// A matching manifest `chain_id` seeds every closed epoch; a mismatching one
/// is rejected before any write, leaving `epochs_v2` untouched.
#[sim_test]
async fn epoch_info_backfill_verifies_chain_before_seeding() {
    // Long epoch duration so the epoch only advances when forced, keeping the
    // closed-epoch set stable across the assertions.
    let test_cluster = TestClusterBuilder::new()
        .with_epoch_duration_ms(600_000)
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

    // This node's chain id — what a same-chain snapshot's manifest carries.
    let expected_chain_id = state.get_chain_identifier();
    let system_state_bytes = bcs::to_bytes(
        &state
            .get_iota_system_state_object_for_testing()
            .expect("current system state"),
    )
    .expect("system state must BCS-encode");

    // SUCCESS: a matching chain_id seeds every closed epoch into a fresh store.
    let ok_tmp = tempfile::tempdir().unwrap();
    let ok_grpc = GrpcIndexesStore::new_without_init(ok_tmp.path().to_path_buf());
    iota_snapshot::verify_and_seed_epochs_v2(
        &ok_grpc,
        real_epoch_info(&checkpoint_store, &system_state_bytes, current_epoch),
        expected_chain_id,
        expected_chain_id,
    )
    .expect("a same-chain snapshot must seed");
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
    iota_snapshot::verify_and_seed_epochs_v2(
        &ok_grpc,
        real_epoch_info(&checkpoint_store, &system_state_bytes, current_epoch),
        expected_chain_id,
        expected_chain_id,
    )
    .expect("re-seeding a covered store must succeed as a no-op");
    assert_eq!(
        ok_grpc.highest_indexed_epoch().unwrap(),
        watermark,
        "re-seeding must leave the watermark unchanged"
    );

    // FAILURE: a different chain_id is rejected and NOTHING is written.
    let bad_tmp = tempfile::tempdir().unwrap();
    let bad_grpc = GrpcIndexesStore::new_without_init(bad_tmp.path().to_path_buf());
    let err = iota_snapshot::verify_and_seed_epochs_v2(
        &bad_grpc,
        real_epoch_info(&checkpoint_store, &system_state_bytes, current_epoch),
        ChainIdentifier::default(),
        expected_chain_id,
    )
    .expect_err("a wrong-network snapshot must be rejected");
    assert!(
        err.to_string().contains("chain_id"),
        "expected a chain_id mismatch error, got: {err}"
    );
    assert!(
        bad_grpc.highest_indexed_epoch().unwrap().is_none(),
        "verification failure must leave the epochs_v2 watermark unset"
    );
    for epoch in 0..current_epoch {
        assert!(
            bad_grpc.get_epoch_info(epoch).unwrap().is_none(),
            "no epoch row may be written when verification fails"
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
    assert!(state.current_epoch_for_testing() >= 2);

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

/// Build a real `EpochInfo` for closed epochs `[0, current_epoch)` from the
/// node's end-of-epoch summaries (the conversion reads each summary's epoch and
/// end-of-epoch data). `start_system_state` only needs to BCS-decode, so the
/// current system state is reused for every entry.
fn real_epoch_info(
    checkpoint_store: &CheckpointStore,
    start_system_state: &[u8],
    current_epoch: EpochId,
) -> EpochInfo {
    let entries = (0..current_epoch)
        .map(|epoch| {
            let last_checkpoint = checkpoint_store
                .get_epoch_last_checkpoint(epoch)
                .unwrap()
                .unwrap_or_else(|| panic!("missing last checkpoint for closed epoch {epoch}"));
            EpochInfoV1Entry {
                epoch,
                start_checkpoint: 0,
                start_system_state: start_system_state.to_vec(),
                last_checkpoint_summary: last_checkpoint.into_inner(),
                end_of_epoch_tx_events: Default::default(),
            }
        })
        .collect();
    EpochInfo::V1(EpochInfoV1 { entries })
}

/// A minimal `epochs_v2` row standing in for one written by a snapshot restore.
/// Only `epoch` and a decodable `system_state` matter here — the end-of-epoch
/// fields are left `None` since the test asserts row survival, not
/// completeness.
fn restored_sentinel_row(epoch: u64) -> EpochInfoV2 {
    EpochInfoV2 {
        epoch,
        protocol_version: 1,
        start_timestamp_ms: 0,
        end_timestamp_ms: None,
        start_checkpoint: 0,
        end_checkpoint: None,
        reference_gas_price: 0,
        system_state: IotaSystemState::for_testing(epoch, 1),
        last_checkpoint_summary: None,
        end_of_epoch_tx_events: None,
    }
}
