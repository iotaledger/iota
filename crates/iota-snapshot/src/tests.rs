// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Write,
    num::NonZeroUsize,
    sync::{Arc, OnceLock},
};

use byteorder::{BigEndian, ByteOrder};
use fastcrypto::{
    hash::{HashFunction, MultisetHash, Sha3_256},
    traits::KeyPair,
};
use futures::future::AbortHandle;
use indicatif::MultiProgress;
use iota_config::object_storage_config::{ObjectStoreConfig, ObjectStoreType};
use iota_core::{
    authority::authority_store_tables::AuthorityPerpetualTables,
    global_state_hasher::GlobalStateHasher,
    grpc_indexes::{GRPC_INDEXES_DIR, GrpcIndexesStore, OwnerTypeFilter},
};
use iota_node_storage::GrpcIndexes;
use iota_sdk_types::{GasCostSummary, ObjectId};
use iota_types::{
    base_types::IotaAddress,
    committee::{Committee, EpochId},
    crypto::AuthorityKeyPair,
    digests::{ChainIdentifier, TransactionDigest},
    effects::{TransactionEffects, TransactionEffectsExtForTesting, TransactionEvents},
    global_state_hash::GlobalStateHash,
    iota_system_state::IotaSystemState,
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContents, CheckpointSummary, ECMHLiveObjectSetDigest,
        EndOfEpochData, SignedCheckpointSummary,
    },
    object::Object,
    storage::{
        CoinInfo, DynamicFieldIteratorItem, EpochInfoV2, OwnedObjectCursor,
        OwnedObjectIteratorItem, PackageVersionIteratorItem, TransactionInfo,
        error::Result as StorageResult,
    },
};

use crate::{
    EPOCH_INFO_FILE_MAGIC, EpochInfo, EpochInfoV1, EpochInfoV1Entry, FileCompression, FileMetadata,
    FileType, MAGIC_BYTES, MANIFEST_FILE_MAGIC, Manifest, ManifestV2, OBJECT_REF_BYTES,
    reader::StateSnapshotReaderV1, restore::RestoreWithGrpcIndexes, verify_epoch_info_chain,
    writer::StateSnapshotWriterV1,
};

/// In-memory `GrpcIndexes` stub for snapshot tests.
///
/// Pre-populates `epochs_v2` rows for a contiguous range `[0..=highest]`
/// with empty-but-structurally-valid `EpochInfoV2`s and advances the
/// `EpochIndexed` watermark to `highest`. Lets snapshot-writer tests satisfy
/// the watermark precondition without standing up a full RocksDB-backed
/// `IndexStoreTables`. Every method other than the two `epoch` paths
/// returns `None`/empty iterators — tests that exercise other surfaces of
/// `GrpcIndexes` should not use this stub.
struct TestGrpcIndexes {
    entries: HashMap<EpochId, EpochInfoV2>,
    highest: Option<EpochId>,
}

impl TestGrpcIndexes {
    /// Synthetic state for the writer's `Some(..)` path: every epoch in
    /// `[0..=highest]` is fully populated and the watermark is set to
    /// `highest`. The writer only reads `[0, snapshot_epoch]`, so this
    /// omits the start-of-epoch-only `highest + 1` row a real index would
    /// carry.
    fn with_epochs_through(highest: EpochId) -> Arc<dyn GrpcIndexes> {
        let mut entries = HashMap::new();
        for epoch in 0..=highest {
            entries.insert(epoch, fully_populated_epoch_info(epoch));
        }
        Arc::new(TestGrpcIndexes {
            entries,
            highest: Some(highest),
        })
    }

    /// Stub where the `EpochIndexed` watermark is absent (no epoch has
    /// been fully indexed yet). Drives the writer's watermark
    /// precondition into the "no rows" failure branch.
    fn empty() -> Arc<dyn GrpcIndexes> {
        Arc::new(TestGrpcIndexes {
            entries: HashMap::new(),
            highest: None,
        })
    }

    /// Stub with the watermark set to `highest` but no rows populated.
    /// The watermark precondition fires before any row is read, so this
    /// is sufficient to drive the "watermark below snapshot_epoch"
    /// failure branch.
    fn watermark_only(highest: EpochId) -> Arc<dyn GrpcIndexes> {
        Arc::new(TestGrpcIndexes {
            entries: HashMap::new(),
            highest: Some(highest),
        })
    }
}

/// Test-fixture system state, reused as a row's decoded `system_state` and as
/// the raw payload in `test_next_epoch_start_system_state_objects`.
/// Distinctive field values make a writer bug that swaps or zeroes a field
/// surface as a specific mismatch rather than silently passing.
fn test_system_state() -> IotaSystemState {
    // Distinctive `epoch` + `protocol_version` so a bug that swaps fields
    // or zeroes them surfaces as a specific mismatch.
    let mut state = IotaSystemState::for_testing(0x1234_5678, 0x9ABC_DEF0);
    // Set distinctive non-zero values `for_testing` zeroes.
    let IotaSystemState::V1(inner) = &mut state else {
        unreachable!("for_testing builds a V1 system state");
    };
    inner.reference_gas_price = 0x0BAD_F00D;
    inner.epoch_start_timestamp_ms = 0x00C0_FFEE;
    state
}

/// One validator set for the whole test process, shared by every fixture:
/// summaries signed in one helper must verify against committees built in
/// another. (`new_simple_test_committee` is fixed-seed deterministic; the
/// `OnceLock` just makes the sharing explicit and skips repeated keygen.)
fn test_validator_set() -> &'static (Committee, Vec<AuthorityKeyPair>) {
    static SET: OnceLock<(Committee, Vec<AuthorityKeyPair>)> = OnceLock::new();
    SET.get_or_init(Committee::new_simple_test_committee)
}

/// The fixed validator set's committee at `epoch` (the set never rotates in
/// these fixtures, only the epoch advances).
fn test_committee_at(epoch: EpochId) -> Committee {
    let (genesis_committee, _) = test_validator_set();
    Committee::new(
        epoch,
        genesis_committee.voting_rights.iter().cloned().collect(),
    )
}

/// An end-of-epoch summary for `epoch`, handing the fixed validator set
/// forward to `epoch + 1`.
fn end_of_epoch_summary(epoch: EpochId) -> CheckpointSummary {
    CheckpointSummary {
        epoch,
        sequence_number: 0,
        network_total_transactions: 0,
        content_digest: Default::default(),
        previous_digest: None,
        epoch_rolling_gas_cost_summary: GasCostSummary::default(),
        end_of_epoch_data: Some(EndOfEpochData {
            next_epoch_committee: test_validator_set().0.voting_rights.clone(),
            next_epoch_protocol_version: 1.into(),
            epoch_commitments: Vec::new(),
            epoch_supply_change: 0,
        }),
        timestamp_ms: 0,
        version_specific_data: Vec::new(),
        checkpoint_commitments: Vec::new(),
    }
}

/// Certify `summary` with the fixed validator set (all four sign — a quorum).
fn certify_summary(summary: CheckpointSummary) -> CertifiedCheckpointSummary {
    let (_, keys) = test_validator_set();
    let committee = test_committee_at(summary.epoch);
    let signatures = keys
        .iter()
        .map(|key| SignedCheckpointSummary::sign(summary.epoch, &summary, key, key.public().into()))
        .collect();
    CertifiedCheckpointSummary::new(summary, signatures, &committee)
        .expect("test summary must certify under the test committee")
}

fn fully_populated_checkpoint_summary(epoch: EpochId) -> CertifiedCheckpointSummary {
    certify_summary(end_of_epoch_summary(epoch))
}

/// Placeholder closing-checkpoint contents for the row/entry fixtures. These
/// fixtures drive the writer/reader round-trip and the on-disk BCS layout,
/// neither of which inspects the proof bundle — full proof-chain verification
/// is exercised against real boundaries in `iota-e2e-tests`.
fn test_checkpoint_contents() -> CheckpointContents {
    CheckpointContents::new_with_digests_only_for_tests(std::iter::empty())
}

/// Placeholder epoch-change effects for the row/entry fixtures.
fn test_end_of_epoch_tx_effects() -> TransactionEffects {
    TransactionEffects::new_empty_v1_for_testing(TransactionDigest::ZERO)
}

/// Placeholder raw start-state object bytes for the row/entry fixtures —
/// a recognizable, distinct payload so a misordered field surfaces.
fn test_next_epoch_start_system_state_objects() -> Vec<Vec<u8>> {
    vec![bcs::to_bytes(&test_system_state()).expect("test_system_state must BCS-encode")]
}

fn fully_populated_epoch_info(epoch: EpochId) -> EpochInfoV2 {
    EpochInfoV2 {
        epoch,
        start_checkpoint: 0,
        start_timestamp_ms: 0,
        system_state: test_system_state(),
        epoch_close_proof: Some(fully_populated_snapshot_epoch_entry(epoch)),
    }
}

/// The close-of-epoch proof embedded in [`fully_populated_epoch_info`] and
/// written to the `EPOCH_INFO` file; used by tests that exercise the on-disk
/// `EpochInfoV1Entry` directly (BCS round-trip of the file body).
fn fully_populated_snapshot_epoch_entry(epoch: EpochId) -> EpochInfoV1Entry {
    EpochInfoV1Entry {
        last_checkpoint_summary: fully_populated_checkpoint_summary(epoch),
        last_checkpoint_contents: test_checkpoint_contents(),
        end_of_epoch_tx_effects: test_end_of_epoch_tx_effects(),
        end_of_epoch_tx_events: TransactionEvents::default(),
        next_epoch_start_system_state_objects: test_next_epoch_start_system_state_objects(),
    }
}

impl GrpcIndexes for TestGrpcIndexes {
    fn get_epoch_info(&self, epoch: EpochId) -> StorageResult<Option<EpochInfoV2>> {
        Ok(self.entries.get(&epoch).cloned())
    }

    fn highest_indexed_epoch(&self) -> StorageResult<Option<EpochId>> {
        Ok(self.highest)
    }

    fn get_transaction_info(
        &self,
        _digest: &TransactionDigest,
    ) -> StorageResult<Option<TransactionInfo>> {
        Ok(None)
    }

    fn account_owned_objects_info_iter(
        &self,
        _owner: iota_types::base_types::IotaAddress,
        _cursor: Option<&OwnedObjectCursor>,
        _object_type: Option<iota_sdk_types::StructTag>,
    ) -> StorageResult<Box<dyn Iterator<Item = OwnedObjectIteratorItem> + '_>> {
        Ok(Box::new(std::iter::empty()))
    }

    fn dynamic_field_iter(
        &self,
        _parent: ObjectId,
        _cursor: Option<ObjectId>,
    ) -> StorageResult<Box<dyn Iterator<Item = DynamicFieldIteratorItem> + '_>> {
        Ok(Box::new(std::iter::empty()))
    }

    fn get_coin_info(
        &self,
        _coin_type: &iota_sdk_types::StructTag,
    ) -> StorageResult<Option<CoinInfo>> {
        Ok(None)
    }

    fn package_versions_iter(
        &self,
        _original_package_id: ObjectId,
        _cursor: Option<u64>,
    ) -> StorageResult<Box<dyn Iterator<Item = PackageVersionIteratorItem> + '_>> {
        Ok(Box::new(std::iter::empty()))
    }
}

pub fn insert_keys(
    db: &AuthorityPerpetualTables,
    total_unique_object_ids: u64,
) -> Result<(), anyhow::Error> {
    let mut id = ObjectId::ZERO;
    for _ in 0..total_unique_object_ids {
        let object = Object::immutable_with_id_for_testing(id);
        // Use a concrete `Some(0)` so the snapshot writer's V1-rejection
        // check passes. The exact value is irrelevant for the round-trip
        // tests, which compare object references rather than checkpoints.
        db.insert_store_object_v2_test_only(object, Some(0))?;
        id = id.next_lexicographical();
    }
    Ok(())
}

fn compare_live_objects(
    db1: &AuthorityPerpetualTables,
    db2: &AuthorityPerpetualTables,
) -> Result<(), anyhow::Error> {
    let mut object_set_1 = HashSet::new();
    let mut object_set_2 = HashSet::new();
    for live_object in db1.iter_live_object_set() {
        object_set_1.insert(live_object.object_reference());
    }
    for live_object in db2.iter_live_object_set() {
        object_set_2.insert(live_object.object_reference());
    }
    assert_eq!(object_set_1, object_set_2);
    Ok(())
}

fn accumulate_live_object_set(perpetual_db: &AuthorityPerpetualTables) -> GlobalStateHash {
    let mut acc = GlobalStateHash::default();
    perpetual_db.iter_live_object_set().for_each(|live_object| {
        GlobalStateHasher::accumulate_live_object(&mut acc, &live_object);
    });
    acc
}

/// Writes a snapshot with `num_objects` live objects to a temp remote store,
/// reads it back into a fresh perpetual DB, and asserts the live object set
/// round-trips.
async fn snapshot_round_trip(
    tmp_dir: &std::path::Path,
    num_objects: u64,
    file_compression: FileCompression,
) -> Result<(), anyhow::Error> {
    let local_store_config = ObjectStoreConfig {
        object_store: Some(ObjectStoreType::File),
        directory: Some(tmp_dir.join("local_dir")),
        ..Default::default()
    };
    let remote_store_config = ObjectStoreConfig {
        object_store: Some(ObjectStoreType::File),
        directory: Some(tmp_dir.join("remote_dir")),
        ..Default::default()
    };

    let snapshot_writer = StateSnapshotWriterV1::new(
        &local_store_config,
        &remote_store_config,
        TestGrpcIndexes::with_epochs_through(0),
        ChainIdentifier::default(),
        file_compression,
        NonZeroUsize::new(1).unwrap(),
    )
    .await?;
    let perpetual_db = Arc::new(AuthorityPerpetualTables::open(&tmp_dir.join("db"), None));
    insert_keys(&perpetual_db, num_objects)?;
    let root_accumulator =
        ECMHLiveObjectSetDigest::from(accumulate_live_object_set(&perpetual_db).digest());
    snapshot_writer
        .write_internal(0, perpetual_db.clone(), root_accumulator)
        .await?;

    // On-disk size assertion: with no compression the uploaded `.ref` file
    // is exactly `MAGIC_BYTES + num_objects * OBJECT_REF_BYTES`. A bug that
    // wrote records of the wrong size would still pass the round-trip if
    // writer and reader agreed on the wrong size. Reads from the remote
    // store, since `sync_file_to_remote` removes the local copy after upload.
    if file_compression == FileCompression::None && num_objects > 0 {
        let ref_file = tmp_dir.join("remote_dir").join("epoch_0").join("1_1.ref");
        let actual_size = fs::metadata(&ref_file)?.len() as usize;
        let expected_size = MAGIC_BYTES + (num_objects as usize) * OBJECT_REF_BYTES;
        assert_eq!(
            actual_size, expected_size,
            "ref-file on-disk size mismatch: expected {expected_size}, got {actual_size}"
        );
    }

    // Lock the EPOCH_INFO file's on-disk shape: 4-byte big-endian magic
    // followed by `bcs(EpochInfo)`. The restore tool consumes this via
    // `read_epoch_info`, so a writer bug (bad magic/filename/encoding) would
    // otherwise only surface post-deploy. Gated on `None` compression so the
    // raw on-disk bytes are readable directly.
    if file_compression == FileCompression::None {
        let epoch_info_file = tmp_dir
            .join("remote_dir")
            .join("epoch_0")
            .join("EPOCH_INFO");
        let bytes = fs::read(&epoch_info_file)?;
        assert!(
            bytes.len() >= MAGIC_BYTES,
            "EPOCH_INFO file is shorter than the magic header: {} bytes",
            bytes.len()
        );
        let magic = BigEndian::read_u32(&bytes[..MAGIC_BYTES]);
        assert_eq!(
            magic, EPOCH_INFO_FILE_MAGIC,
            "EPOCH_INFO magic mismatch: got {magic:#x}, expected {EPOCH_INFO_FILE_MAGIC:#x}"
        );
        let decoded: EpochInfo = bcs::from_bytes(&bytes[MAGIC_BYTES..])?;
        let EpochInfo::V1(decoded_v1) = decoded;
        // `TestGrpcIndexes::with_epochs_through(0)` provides exactly one
        // populated entry for epoch 0, so the snapshot's EPOCH_INFO must
        // carry exactly one entry.
        assert_eq!(
            decoded_v1.entries.len(),
            1,
            "expected `entries` of length 1 for snapshot at epoch 0"
        );
        let entry = &decoded_v1.entries[0];
        // Bit-identical round-trip of the raw start-state object bytes. The
        // writer copies the proof bundle's `next_epoch_start_system_state_objects`
        // verbatim; the assertion locks that the bytes on disk equal the
        // fixture's. A writer bug that truncated, padded, or re-encoded the
        // field would change these bytes and fail here, even though the outer
        // BCS round-trip would still succeed.
        assert_eq!(
            entry.next_epoch_start_system_state_objects,
            test_next_epoch_start_system_state_objects(),
            "next_epoch_start_system_state_objects did not round-trip bit-identical \
             through the snapshot"
        );
    }

    let local_store_restore_config = ObjectStoreConfig {
        object_store: Some(ObjectStoreType::File),
        directory: Some(tmp_dir.join("local_dir_restore")),
        ..Default::default()
    };
    let mut snapshot_reader = StateSnapshotReaderV1::new(
        0,
        &remote_store_config,
        &local_store_restore_config,
        NonZeroUsize::new(1).unwrap(),
        MultiProgress::new(),
        false, // skip_reset_local_store
    )
    .await?;
    let restored_perpetual_db = AuthorityPerpetualTables::open(&tmp_dir.join("restored_db"), None);
    let (_abort_handle, abort_registration) = AbortHandle::new_pair();
    snapshot_reader
        .read(&restored_perpetual_db, abort_registration, None)
        .await?;
    compare_live_objects(&perpetual_db, &restored_perpetual_db)?;

    // Snapshot is at epoch 0, so EPOCH_INFO has exactly one entry, which must
    // round-trip into an `EpochInfoV2`.
    let epoch_info = snapshot_reader
        .read_epoch_info()
        .await
        .expect("read_epoch_info");
    assert_eq!(
        epoch_info.entries().len(),
        1,
        "expected one entry per epoch"
    );
    for (i, entry) in epoch_info.entries().iter().enumerate() {
        assert_eq!(entry.last_checkpoint_summary.epoch(), i as u64);
    }
    Ok(())
}

/// Writes the EPOCH_INFO file for `[0, snapshot_epoch]`, reads it back, and
/// asserts every entry round-trips well-formed (one per epoch, in order, with a
/// populated proof bundle) — the writer/reader half of the restore path.
async fn epoch_info_backfill_round_trip(
    tmp_dir: &std::path::Path,
    snapshot_epoch: EpochId,
) -> Result<(), anyhow::Error> {
    let local_store_config = ObjectStoreConfig {
        object_store: Some(ObjectStoreType::File),
        directory: Some(tmp_dir.join("local_dir")),
        ..Default::default()
    };
    let remote_store_config = ObjectStoreConfig {
        object_store: Some(ObjectStoreType::File),
        directory: Some(tmp_dir.join("remote_dir")),
        ..Default::default()
    };

    // 1. CREATE: write the EPOCH_INFO file for epochs `[0, snapshot_epoch]`. The
    //    path is object-independent, so an empty perpetual DB suffices.
    let snapshot_writer = StateSnapshotWriterV1::new(
        &local_store_config,
        &remote_store_config,
        TestGrpcIndexes::with_epochs_through(snapshot_epoch),
        ChainIdentifier::default(),
        FileCompression::Zstd,
        NonZeroUsize::new(1).unwrap(),
    )
    .await?;
    let perpetual_db = Arc::new(AuthorityPerpetualTables::open(&tmp_dir.join("db"), None));
    insert_keys(&perpetual_db, 0)?;
    let root_accumulator =
        ECMHLiveObjectSetDigest::from(accumulate_live_object_set(&perpetual_db).digest());
    snapshot_writer
        .write_internal(snapshot_epoch, perpetual_db, root_accumulator)
        .await?;

    // 2. LOAD via `read_epoch_info_only` (the running-node background backfill
    //    path): downloads only MANIFEST + EPOCH_INFO, verifies sha3 + magic.
    let (chain_id, epoch_info) =
        StateSnapshotReaderV1::read_epoch_info_only(snapshot_epoch, &remote_store_config).await?;
    assert_eq!(
        chain_id,
        ChainIdentifier::default(),
        "manifest chain_id must round-trip through the snapshot"
    );
    let expected_len = (snapshot_epoch + 1) as usize;
    assert_eq!(
        epoch_info.entries().len(),
        expected_len,
        "EPOCH_INFO must carry one entry per epoch in [0, {snapshot_epoch}]"
    );

    // 3. ASSERT the on-disk entries round-trip well-formed: one per epoch in order,
    //    each carrying its summary and a populated proof bundle. The chain-verified
    //    seed into `epochs_v2` (which derives each row's start state from the
    //    previous entry) needs a real boundary and is exercised in
    //    `iota-e2e-tests`.
    for (epoch, entry) in epoch_info.entries().iter().enumerate() {
        let epoch = epoch as EpochId;
        assert_eq!(
            entry.last_checkpoint_summary.epoch(),
            epoch,
            "epoch {epoch}: round-tripped summary carries the wrong epoch"
        );
        assert!(
            !entry.next_epoch_start_system_state_objects.is_empty(),
            "epoch {epoch}: next_epoch_start_system_state_objects is empty"
        );
    }
    Ok(())
}

/// Runs the snapshot writer against `stub` and returns the resulting
/// error. Used by the watermark-precondition sub-cases below; collapses
/// the local/remote-store + empty perpetual-DB boilerplate. The watermark
/// check is the first thing `write_internal` does, so the DB content is
/// irrelevant to these cases — an empty DB suffices.
async fn writer_with_stub_returns_err(
    tmp_dir: &std::path::Path,
    snapshot_epoch: u64,
    stub: Arc<dyn GrpcIndexes>,
) -> anyhow::Error {
    let local_store_config = ObjectStoreConfig {
        object_store: Some(ObjectStoreType::File),
        directory: Some(tmp_dir.join("local_dir")),
        ..Default::default()
    };
    let remote_store_config = ObjectStoreConfig {
        object_store: Some(ObjectStoreType::File),
        directory: Some(tmp_dir.join("remote_dir")),
        ..Default::default()
    };
    let snapshot_writer = StateSnapshotWriterV1::new(
        &local_store_config,
        &remote_store_config,
        stub,
        ChainIdentifier::default(),
        FileCompression::None,
        NonZeroUsize::new(1).unwrap(),
    )
    .await
    .expect("snapshot writer setup");
    let perpetual_db = Arc::new(AuthorityPerpetualTables::open(&tmp_dir.join("db"), None));
    let root_accumulator =
        ECMHLiveObjectSetDigest::from(accumulate_live_object_set(&perpetual_db).digest());
    snapshot_writer
        .write_internal(snapshot_epoch, perpetual_db, root_accumulator)
        .await
        .expect_err("snapshot writer must reject when watermark is insufficient")
}

/// Populated case with compression — exercises the production path.
#[tokio::test]
async fn snapshot_round_trip_populated_zstd() -> Result<(), anyhow::Error> {
    let dir = iota_common::tempdir();
    snapshot_round_trip(dir.path(), 1000, FileCompression::Zstd).await
}

/// EPOCH_INFO write → read entry round-trip across two epochs.
#[tokio::test]
async fn snapshot_epoch_info_backfill_round_trip() -> Result<(), anyhow::Error> {
    let dir = iota_common::tempdir();
    epoch_info_backfill_round_trip(dir.path(), 2).await
}

/// Restoring through [`RestoreWithGrpcIndexes`] must build the gRPC
/// live-state indexes from the same object stream that fills the perpetual
/// tables: the restored live object set matches the source, address-owned
/// objects come back owner-indexed, and `finalize_restore` leaves the store
/// initialized.
#[tokio::test]
async fn snapshot_restore_builds_grpc_indexes() -> Result<(), anyhow::Error> {
    let dir = iota_common::tempdir();
    let tmp_dir = dir.path();

    let local_store_config = ObjectStoreConfig {
        object_store: Some(ObjectStoreType::File),
        directory: Some(tmp_dir.join("local_dir")),
        ..Default::default()
    };
    let remote_store_config = ObjectStoreConfig {
        object_store: Some(ObjectStoreType::File),
        directory: Some(tmp_dir.join("remote_dir")),
        ..Default::default()
    };
    let snapshot_writer = StateSnapshotWriterV1::new(
        &local_store_config,
        &remote_store_config,
        TestGrpcIndexes::with_epochs_through(0),
        ChainIdentifier::default(),
        FileCompression::Zstd,
        NonZeroUsize::new(1).unwrap(),
    )
    .await?;

    // A handful of address-owned objects, so the owner index has something
    // to prove (the immutable objects of the other round-trip tests are
    // intentionally not owner-indexed).
    let perpetual_db = Arc::new(AuthorityPerpetualTables::open(&tmp_dir.join("db"), None));
    let owner = IotaAddress::from_u16(7);
    let mut owned_ids = HashSet::new();
    let mut id = ObjectId::ZERO;
    for _ in 0..4 {
        perpetual_db.insert_store_object_v2_test_only(
            Object::with_id_owner_for_testing(id, owner),
            Some(0),
        )?;
        owned_ids.insert(id);
        id = id.next_lexicographical();
    }
    let root_accumulator =
        ECMHLiveObjectSetDigest::from(accumulate_live_object_set(&perpetual_db).digest());
    snapshot_writer
        .write_internal(0, perpetual_db.clone(), root_accumulator)
        .await?;

    let local_store_restore_config = ObjectStoreConfig {
        object_store: Some(ObjectStoreType::File),
        directory: Some(tmp_dir.join("local_dir_restore")),
        ..Default::default()
    };
    let mut snapshot_reader = StateSnapshotReaderV1::new(
        0,
        &remote_store_config,
        &local_store_restore_config,
        NonZeroUsize::new(1).unwrap(),
        MultiProgress::new(),
        false, // skip_reset_local_store
    )
    .await?;

    let restored_perpetual_db = AuthorityPerpetualTables::open(&tmp_dir.join("restored_db"), None);
    let restored_grpc = GrpcIndexesStore::new_without_init(tmp_dir.join(GRPC_INDEXES_DIR));
    let grpc_restorer = restored_grpc.live_object_restorer();
    let (_abort_handle, abort_registration) = AbortHandle::new_pair();
    snapshot_reader
        .read_to_db(
            &RestoreWithGrpcIndexes::new(&restored_perpetual_db, &grpc_restorer),
            abort_registration,
            None,
        )
        .await?;
    grpc_restorer.finish()?;

    // The tee must not disturb the perpetual-tables restore.
    compare_live_objects(&perpetual_db, &restored_perpetual_db)?;

    // Every address-owned object is owner-indexed in the restored gRPC store.
    let restored_ids: HashSet<ObjectId> = restored_grpc
        .owner_iter(owner, None, OwnerTypeFilter::None)?
        .map(|entry| entry.map(|(key, _)| key.object_id))
        .collect::<Result<_, _>>()?;
    assert_eq!(restored_ids, owned_ids);

    // Finalize the restored store. The chain-verify + epoch-row seed step
    // needs a real boundary's proof bundle and is exercised in
    // `iota-e2e-tests`.
    restored_grpc.finalize_restore(0)?;
    Ok(())
}

/// Empty-DB case.
#[tokio::test]
async fn snapshot_round_trip_empty() -> Result<(), anyhow::Error> {
    let dir = iota_common::tempdir();
    snapshot_round_trip(dir.path(), 0, FileCompression::Zstd).await
}

/// Uncompressed case so the ref-file on-disk size assertion can run directly
/// against the staged file.
#[tokio::test]
async fn snapshot_round_trip_uncompressed() -> Result<(), anyhow::Error> {
    let dir = iota_common::tempdir();
    snapshot_round_trip(dir.path(), 100, FileCompression::None).await
}

/// Per-object `previous_transaction_checkpoint` round-trip end-to-end:
/// stamped on `StoreObjectV2` at insert time, surfaced by `LiveSetIter`,
/// BCS-encoded into `LiveObject` records in the `.obj` files, decoded by
/// `LiveObjectIter`, and re-stamped onto the restored DB by
/// `bulk_insert_live_objects`. Without this, a regression that e.g.
/// reverted the restore path to stamping `None` would still pass
/// `snapshot_round_trip` (which only compares `object_reference`s) — this
/// test is the focused canary for the per-object checkpoint contract.
#[tokio::test]
async fn snapshot_round_trip_per_object_checkpoint() -> Result<(), anyhow::Error> {
    let dir = iota_common::tempdir();
    let tmp_dir = dir.path();

    let local_store_config = ObjectStoreConfig {
        object_store: Some(ObjectStoreType::File),
        directory: Some(tmp_dir.join("local_dir")),
        ..Default::default()
    };
    let remote_store_config = ObjectStoreConfig {
        object_store: Some(ObjectStoreType::File),
        directory: Some(tmp_dir.join("remote_dir")),
        ..Default::default()
    };

    let snapshot_writer = StateSnapshotWriterV1::new(
        &local_store_config,
        &remote_store_config,
        TestGrpcIndexes::with_epochs_through(0),
        ChainIdentifier::default(),
        FileCompression::None,
        NonZeroUsize::new(1).unwrap(),
    )
    .await?;

    let perpetual_db = Arc::new(AuthorityPerpetualTables::open(&tmp_dir.join("db"), None));

    // Insert objects with distinct, recognizable per-object checkpoints.
    // The pattern avoids `0` (the default a buggy restore might stamp).
    // Each object gets a unique value so a swap or off-by-one bug surfaces
    // as a specific mismatch rather than a uniform clobber.
    const NUM_OBJECTS: u64 = 32;
    const CHECKPOINT_BASE: u64 = 0xC0FF_EE00_0000_0000;
    let mut expected: HashMap<ObjectId, Option<u64>> = HashMap::new();
    let mut id = ObjectId::ZERO;
    for i in 0..NUM_OBJECTS {
        let object = Object::immutable_with_id_for_testing(id);
        let checkpoint = CHECKPOINT_BASE | i;
        perpetual_db.insert_store_object_v2_test_only(object, Some(checkpoint))?;
        expected.insert(id, Some(checkpoint));
        id = id.next_lexicographical();
    }

    let root_accumulator =
        ECMHLiveObjectSetDigest::from(accumulate_live_object_set(&perpetual_db).digest());
    snapshot_writer
        .write_internal(0, perpetual_db.clone(), root_accumulator)
        .await?;

    let local_store_restore_config = ObjectStoreConfig {
        object_store: Some(ObjectStoreType::File),
        directory: Some(tmp_dir.join("local_dir_restore")),
        ..Default::default()
    };
    let mut snapshot_reader = StateSnapshotReaderV1::new(
        0,
        &remote_store_config,
        &local_store_restore_config,
        NonZeroUsize::new(1).unwrap(),
        MultiProgress::new(),
        false,
    )
    .await?;
    let restored_perpetual_db = AuthorityPerpetualTables::open(&tmp_dir.join("restored_db"), None);
    let (_abort_handle, abort_registration) = AbortHandle::new_pair();
    snapshot_reader
        .read(&restored_perpetual_db, abort_registration, None)
        .await?;

    // Read every restored row through `LiveSetIter` and compare against
    // the values written before the snapshot was taken.
    let restored: HashMap<ObjectId, Option<u64>> = restored_perpetual_db
        .iter_live_object_set()
        .map(|live_object| {
            (
                live_object.object_id(),
                live_object.previous_transaction_checkpoint,
            )
        })
        .collect();
    assert_eq!(
        restored, expected,
        "per-object previous_transaction_checkpoint did not round-trip through the snapshot"
    );
    Ok(())
}

/// Asserts that the snapshot writer rejects a perpetual DB that still
/// contains rows lifted from pre-V2 on-disk format
/// (`previous_transaction_checkpoint == None`). Pre-V2 rows never recorded the
/// containing checkpoint and there is no way to recover it; emitting them into
/// the snapshot file would force downstream consumers to handle unknown
/// checkpoints forever. The writer must fail loudly so an operator who hasn't
/// synced from genesis under V2 sees the problem at publish time, not after
/// the bad snapshot has been uploaded. This test inserts the V2 row with
/// `None` directly to isolate the writer's rejection check; the end-to-end
/// V1->lift->reject pipeline is covered by
/// `snapshot_writer_rejects_literal_v1_row`.
#[tokio::test]
async fn snapshot_writer_rejects_lifted_v1_row() -> Result<(), anyhow::Error> {
    let dir = iota_common::tempdir();
    let tmp_dir = dir.path();

    let local_store_config = ObjectStoreConfig {
        object_store: Some(ObjectStoreType::File),
        directory: Some(tmp_dir.join("local_dir")),
        ..Default::default()
    };
    let remote_dir = tmp_dir.join("remote_dir");
    let remote_store_config = ObjectStoreConfig {
        object_store: Some(ObjectStoreType::File),
        directory: Some(remote_dir.clone()),
        ..Default::default()
    };

    let snapshot_writer = StateSnapshotWriterV1::new(
        &local_store_config,
        &remote_store_config,
        TestGrpcIndexes::with_epochs_through(0),
        ChainIdentifier::default(),
        FileCompression::None,
        NonZeroUsize::new(1).unwrap(),
    )
    .await?;

    let perpetual_db = Arc::new(AuthorityPerpetualTables::open(&tmp_dir.join("db"), None));
    // Mix a normal V2 row (Some(seq)) with a lifted-V1 row (None) so the
    // assertion is genuinely about the lifted row triggering the error,
    // not about the DB being empty.
    perpetual_db.insert_store_object_v2_test_only(
        Object::immutable_with_id_for_testing(ObjectId::ZERO),
        Some(42),
    )?;
    let lifted_id = ObjectId::ZERO.next_lexicographical();
    perpetual_db
        .insert_store_object_v2_test_only(Object::immutable_with_id_for_testing(lifted_id), None)?;

    let root_accumulator =
        ECMHLiveObjectSetDigest::from(accumulate_live_object_set(&perpetual_db).digest());
    let err = snapshot_writer
        .write_internal(0, perpetual_db, root_accumulator)
        .await
        .expect_err("writer must reject a DB containing a lifted V1 row");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("lifted from a pre-V2 store row"),
        "unexpected error chain for lifted V1 row: {msg}"
    );
    // No `.obj` or `.ref` files should reach the remote store: the writer
    // bails out before `LiveObjectSetWriterV1::done()` is called (which is
    // the only path that pushes `FileMetadata` onto the upload channel).
    assert_no_bucket_files(&remote_dir);
    Ok(())
}

/// End-to-end variant of `snapshot_writer_rejects_lifted_v1_row`: inserts a
/// literal `StoreObjectWrapper::V1` row directly into the perpetual `objects`
/// map (bypassing `get_store_object`, which always produces V2), then runs
/// the snapshot writer. The on-read `migrate()` step must lift the row to
/// `StoreObjectV2(None)`, `LiveSetIter` must surface the `None`, and the
/// writer must reject the publish — covering the full V1->lift->reject
/// pipeline as a single assertion.
#[tokio::test]
async fn snapshot_writer_rejects_literal_v1_row() -> Result<(), anyhow::Error> {
    let dir = iota_common::tempdir();
    let tmp_dir = dir.path();

    let local_store_config = ObjectStoreConfig {
        object_store: Some(ObjectStoreType::File),
        directory: Some(tmp_dir.join("local_dir")),
        ..Default::default()
    };
    let remote_dir = tmp_dir.join("remote_dir");
    let remote_store_config = ObjectStoreConfig {
        object_store: Some(ObjectStoreType::File),
        directory: Some(remote_dir.clone()),
        ..Default::default()
    };

    let snapshot_writer = StateSnapshotWriterV1::new(
        &local_store_config,
        &remote_store_config,
        TestGrpcIndexes::with_epochs_through(0),
        ChainIdentifier::default(),
        FileCompression::None,
        NonZeroUsize::new(1).unwrap(),
    )
    .await?;

    let perpetual_db = Arc::new(AuthorityPerpetualTables::open(&tmp_dir.join("db"), None));
    // Single literal V1 row. The writer must fail end-to-end.
    perpetual_db
        .insert_store_object_v1_test_only(Object::immutable_with_id_for_testing(ObjectId::ZERO))?;

    let root_accumulator =
        ECMHLiveObjectSetDigest::from(accumulate_live_object_set(&perpetual_db).digest());
    let err = snapshot_writer
        .write_internal(0, perpetual_db, root_accumulator)
        .await
        .expect_err("writer must reject a DB containing a literal V1 row");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("lifted from a pre-V2 store row"),
        "unexpected error chain for literal V1 row: {msg}"
    );
    assert_no_bucket_files(&remote_dir);
    Ok(())
}

/// Watermark precondition: absent watermark rejects the snapshot. Matched
/// against the full anyhow chain via `{err:#}` because `write_internal` wraps
/// the inner error with a context message.
#[tokio::test]
async fn snapshot_writer_rejects_absent_watermark() {
    let dir = iota_common::tempdir();
    let err = writer_with_stub_returns_err(dir.path(), 0, TestGrpcIndexes::empty()).await;
    let msg = format!("{err:#}");
    assert!(
        msg.contains("`EpochIndexed` watermark is absent"),
        "absent-watermark error chain did not match: {msg}"
    );
}

/// Watermark precondition: `Some(h)` with `h < snapshot_epoch` rejects the
/// snapshot. The "watermark is at epoch N, but snapshot_epoch is M" wording is
/// itself part of the operator-facing contract — pin both sides so a rewording
/// that drops one is caught here.
#[tokio::test]
async fn snapshot_writer_rejects_watermark_below_snapshot_epoch() {
    let dir = iota_common::tempdir();
    let err = writer_with_stub_returns_err(dir.path(), 5, TestGrpcIndexes::watermark_only(3)).await;
    let msg = format!("{err:#}");
    assert!(
        msg.contains("`EpochIndexed` watermark is at epoch 3"),
        "too-low watermark error chain did not match (epoch 3): {msg}"
    );
    assert!(
        msg.contains("snapshot_epoch is 5"),
        "too-low watermark error chain did not match (snapshot_epoch 5): {msg}"
    );
}

/// Recursively scans `root` and asserts no `.obj` or `.ref` files exist.
fn assert_no_bucket_files(root: &std::path::Path) {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                assert!(
                    ext != "obj" && ext != "ref",
                    "writer emitted a bucket file despite rejecting the input: {}",
                    path.display()
                );
            }
        }
    }
}

/// Negative test: a V2 manifest without an `EPOCH_INFO` entry must be
/// rejected up-front by `StateSnapshotReaderV1::new`. Locks the contract that
/// the manifest's EPOCH_INFO entry is required.
#[tokio::test]
async fn test_v2_manifest_missing_epoch_info_is_rejected() {
    let tmp_dir = iota_common::tempdir();
    let remote_root = tmp_dir.path().join("remote_dir");
    let epoch_dir = remote_root.join("epoch_0");
    fs::create_dir_all(&epoch_dir).unwrap();

    // Manifest with file_metadata containing a bogus reference entry but no
    // EPOCH_INFO. The reader should reject this up-front.
    let manifest = Manifest::V2(ManifestV2 {
        snapshot_version: 2,
        address_length: ObjectId::LENGTH as u64,
        file_metadata: vec![FileMetadata {
            file_type: FileType::Reference,
            bucket_num: 1,
            part_num: 1,
            file_compression: FileCompression::None,
            sha3_digest: [0u8; 32],
        }],
        epoch: 0,
        chain_id: ChainIdentifier::default(),
    });
    let manifest_path = epoch_dir.join("MANIFEST");
    write_manifest_file(&manifest_path, &manifest).unwrap();

    let remote_store_config = ObjectStoreConfig {
        object_store: Some(ObjectStoreType::File),
        directory: Some(remote_root),
        ..Default::default()
    };
    let local_store_config = ObjectStoreConfig {
        object_store: Some(ObjectStoreType::File),
        directory: Some(tmp_dir.path().join("local_dir_restore")),
        ..Default::default()
    };

    let result = StateSnapshotReaderV1::new(
        0,
        &remote_store_config,
        &local_store_config,
        NonZeroUsize::new(1).unwrap(),
        MultiProgress::new(),
        false,
    )
    .await;
    let err = result
        .err()
        .expect("missing EPOCH_INFO must be rejected by the reader");
    assert!(
        err.to_string()
            .contains("V2 manifest missing required EPOCH_INFO entry"),
        "expected EPOCH_INFO-missing error, got: {err}"
    );
}

/// Writes a freestanding MANIFEST file in the same on-disk format as
/// `StateSnapshotWriterV1::write_manifest`: 4-byte big-endian magic, BCS
/// payload, 32-byte sha3 trailer over (magic + bcs).
fn write_manifest_file(path: &std::path::Path, manifest: &Manifest) -> std::io::Result<()> {
    let mut magic_buf = [0u8; MAGIC_BYTES];
    BigEndian::write_u32(&mut magic_buf, MANIFEST_FILE_MAGIC);
    let body = bcs::to_bytes(manifest).expect("manifest serialization");
    let mut hasher = Sha3_256::default();
    hasher.update(magic_buf);
    hasher.update(&body);
    let sha3 = hasher.finalize().digest;

    let mut f = fs::File::create(path)?;
    f.write_all(&magic_buf)?;
    f.write_all(&body)?;
    f.write_all(&sha3)?;
    f.sync_data()?;
    Ok(())
}

/// Locks the on-disk format of the `EPOCH_INFO` file body: BCS-encoding
/// `EpochInfo::V1` must use variant tag `0`, and `entries` must round-trip
/// with its length and per-slot ordering preserved.
///
/// The end-to-end payload (writer → BCS → file → BCS → reader) is covered
/// by `snapshot_round_trip`'s EPOCH_INFO assertion. This test exercises
/// the discriminant and `Vec` framing in isolation so an on-disk-format
/// regression that decouples from the writer (e.g. someone adds an
/// `EpochInfoV2` variant before `V1`) is caught even if the writer path
/// is healthy.
#[test]
fn epoch_info_v1_bcs_round_trip() {
    // Three entries with distinct epoch numbers so a Vec-reordering bug
    // surfaces as a per-slot mismatch below.
    let epoch_info = EpochInfo::V1(EpochInfoV1 {
        entries: vec![
            fully_populated_snapshot_epoch_entry(0),
            fully_populated_snapshot_epoch_entry(1),
            fully_populated_snapshot_epoch_entry(2),
        ],
    });
    let bytes = bcs::to_bytes(&epoch_info).unwrap();
    assert_eq!(
        bytes[0], 0,
        "EpochInfo::V1 must remain at BCS discriminant 0"
    );

    let decoded: EpochInfo = bcs::from_bytes(&bytes).unwrap();
    let EpochInfo::V1(decoded_v1) = decoded;
    assert_eq!(decoded_v1.entries.len(), 3);
    // Per-entry summary carries the epoch number, so this asserts the
    // Vec ordering is preserved across BCS round-trip.
    for (i, entry) in decoded_v1.entries.iter().enumerate() {
        assert_eq!(entry.last_checkpoint_summary.epoch(), i as EpochId);
    }
}

/// Locks the BCS field order of `EpochInfoV1Entry` against silent
/// reordering. BCS encodes struct fields in declaration order, so
/// swapping any two fields would silently change the on-disk EPOCH_INFO
/// file layout and break every existing snapshot consumer.
///
/// Asserts that `bcs(entry)` equals the concatenation of the per-field BCS
/// encodings in declaration order. This both verifies the relative order of
/// the fields and detects any encoding-shape change in the inner types.
#[test]
fn snapshot_epoch_info_field_order_is_locked() {
    let entry = EpochInfoV1Entry {
        last_checkpoint_summary: fully_populated_checkpoint_summary(0),
        last_checkpoint_contents: test_checkpoint_contents(),
        end_of_epoch_tx_effects: test_end_of_epoch_tx_effects(),
        end_of_epoch_tx_events: TransactionEvents::default(),
        // Distinct payload so a misordered field would be obvious.
        next_epoch_start_system_state_objects: vec![vec![0xAA, 0xBB, 0xCC, 0xDD]],
    };

    let entry_bytes = bcs::to_bytes(&entry).expect("entry serialization");

    let mut expected = Vec::with_capacity(entry_bytes.len());
    expected.extend_from_slice(
        &bcs::to_bytes(&entry.last_checkpoint_summary).expect("summary serialization"),
    );
    expected.extend_from_slice(
        &bcs::to_bytes(&entry.last_checkpoint_contents).expect("contents serialization"),
    );
    expected.extend_from_slice(
        &bcs::to_bytes(&entry.end_of_epoch_tx_effects).expect("effects serialization"),
    );
    expected.extend_from_slice(
        &bcs::to_bytes(&entry.end_of_epoch_tx_events).expect("events serialization"),
    );
    expected.extend_from_slice(
        &bcs::to_bytes(&entry.next_epoch_start_system_state_objects)
            .expect("objects serialization"),
    );

    assert_eq!(
        entry_bytes, expected,
        "EpochInfoV1Entry BCS layout changed; re-anchor this test only if \
         the schema change is deliberate and reviewers have signed off"
    );
}

/// A valid `EPOCH_INFO` for epochs `[0, snapshot_epoch]`, certified by the
/// fixed test validator set with the committee handed forward at every close.
fn signed_epoch_info(snapshot_epoch: EpochId) -> EpochInfo {
    EpochInfo::V1(EpochInfoV1 {
        entries: (0..=snapshot_epoch)
            .map(fully_populated_snapshot_epoch_entry)
            .collect(),
    })
}

/// A wrong-network snapshot is rejected on the chain id, before any per-entry
/// work — so this case needs no valid proof bundle. The committee-chain walk,
/// proof-bundle anchoring, and per-link tamper cases require a real epoch
/// boundary and are covered in `iota-e2e-tests`.
#[test]
fn verify_epoch_info_chain_rejects_wrong_chain_id() {
    let err = verify_epoch_info_chain(
        signed_epoch_info(1),
        test_committee_at(0),
        test_system_state(),
        ChainIdentifier::default(),
        ChainIdentifier::from(iota_types::digests::CheckpointDigest::new([7; 32])),
    )
    .expect_err("a foreign chain id must be rejected");
    assert!(err.to_string().contains("chain_id"), "got: {err}");
}

/// Entries must be the contiguous epochs from 0: a first entry that is not
/// epoch 0 is rejected at the contiguity check, before any per-entry proof
/// work — so this needs no valid proof bundle.
#[test]
fn verify_epoch_info_chain_rejects_non_contiguous_entries() {
    let EpochInfo::V1(mut info) = signed_epoch_info(2);
    info.entries.remove(0); // entries now start at epoch 1, not 0
    let err = verify_epoch_info_chain(
        EpochInfo::V1(info),
        test_committee_at(0),
        test_system_state(),
        ChainIdentifier::default(),
        ChainIdentifier::default(),
    )
    .expect_err("non-contiguous entries must be rejected");
    assert!(
        err.to_string().contains("carries a summary for epoch"),
        "got: {err}"
    );
}

/// `epoch_info_v2_row` derives every non-stored `EpochInfoV2` field: the
/// `epoch` from the signed summary, the `end_*` from the embedded entry, the
/// system-state fields from the start system state. The chain-verify path that
/// feeds it real entries is exercised in `iota-e2e-tests`.
#[test]
fn epoch_info_v2_row_derives_fields() {
    use iota_types::iota_system_state::IotaSystemStateTrait;

    let entry = fully_populated_snapshot_epoch_entry(3);
    let system_state = test_system_state();
    let start_checkpoint = 7;
    let row = crate::epoch_info_v2_row(entry.clone(), system_state.clone(), start_checkpoint);

    assert_eq!(row.epoch, 3, "epoch comes from the entry's signed summary");
    assert_eq!(row.start_checkpoint, start_checkpoint);
    // `end_*` derived from the signed last-checkpoint summary.
    assert_eq!(
        row.end_checkpoint(),
        Some(*entry.last_checkpoint_summary.data().sequence_number()),
    );
    assert_eq!(
        row.end_timestamp_ms(),
        Some(entry.last_checkpoint_summary.data().timestamp_ms),
    );
    // System-state fields derived from the start system state.
    assert_eq!(row.protocol_version(), system_state.protocol_version());
    assert_eq!(
        row.reference_gas_price(),
        system_state.reference_gas_price()
    );
    assert_eq!(
        row.start_timestamp_ms,
        system_state.epoch_start_timestamp_ms()
    );
}

/// An open (not-yet-indexed) row has no `epoch_close_proof`, so it is not
/// finalized and its `end_*` helpers return `None`. The finalized values are
/// covered by `epoch_info_v2_row_derives_fields`.
#[test]
fn open_row_has_no_end_fields() {
    let open = EpochInfoV2 {
        epoch_close_proof: None,
        ..fully_populated_epoch_info(2)
    };
    assert!(!open.is_finalized());
    assert_eq!(open.end_checkpoint(), None);
    assert_eq!(open.end_timestamp_ms(), None);
}
