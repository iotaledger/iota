// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Write,
    num::NonZeroUsize,
    sync::Arc,
};

use byteorder::{BigEndian, ByteOrder};
use fastcrypto::hash::{HashFunction, MultisetHash, Sha3_256};
use futures::future::AbortHandle;
use indicatif::MultiProgress;
use iota_config::object_storage_config::{ObjectStoreConfig, ObjectStoreType};
use iota_core::{
    authority::authority_store_tables::AuthorityPerpetualTables,
    global_state_hasher::GlobalStateHasher,
};
use iota_node_storage::GrpcIndexes;
use iota_types::{
    base_types::ObjectID,
    committee::EpochId,
    crypto::AuthorityStrongQuorumSignInfo,
    digests::TransactionDigest,
    effects::TransactionEvents,
    epoch_info::EpochInfoEntry,
    gas::GasCostSummary,
    global_state_hash::GlobalStateHash,
    message_envelope::Envelope,
    messages_checkpoint::{CheckpointSummary, ECMHLiveObjectSetDigest},
    object::Object,
    storage::{
        CoinInfo, DynamicFieldIteratorItem, EpochInfo, OwnedObjectCursor, OwnedObjectIteratorItem,
        PackageVersionIteratorItem, TransactionInfo, error::Result as StorageResult,
    },
};

use crate::{
    EPOCH_INFO_FILE_MAGIC, EpochInfo as SnapshotEpochInfo, EpochInfoV1, FileCompression,
    FileMetadata, FileType, MAGIC_BYTES, MANIFEST_FILE_MAGIC, Manifest, ManifestBody,
    OBJECT_REF_BYTES, reader::StateSnapshotReaderV1, writer::StateSnapshotWriterV1,
};

/// In-memory `GrpcIndexes` stub for snapshot tests.
///
/// Pre-populates `epoch_info` rows for a contiguous range `[0..=highest]`
/// with empty-but-structurally-valid `EpochInfoEntry`s and advances the
/// `EpochIndexed` watermark to `highest`. Lets snapshot-writer tests satisfy
/// the watermark precondition without standing up a full RocksDB-backed
/// `IndexStoreTables`. Every method other than the two `epoch_info` paths
/// returns `None`/empty iterators — tests that exercise other surfaces of
/// `GrpcIndexes` should not use this stub.
struct TestGrpcIndexes {
    entries: HashMap<EpochId, EpochInfoEntry>,
    highest: Option<EpochId>,
}

impl TestGrpcIndexes {
    /// Synthetic state for exercising the writer's `Some(..)` path: every
    /// epoch in `[0..=highest]` is fully populated and the watermark is
    /// advanced to `highest`. In production, epoch `N`'s row is only
    /// finalized when epoch `N+1` is seeded (see the boundary-2 logic in
    /// `grpc_indexes::write_epoch_info_entries`), so a true production
    /// state with `EpochIndexed = highest` would additionally carry a
    /// boundary-1-only row for `highest + 1`. The writer only reads
    /// `[0, snapshot_epoch]`, so that extra row is irrelevant to the
    /// tests here — but the asymmetry is worth flagging so future
    /// readers don't mistake this fixture for a production snapshot.
    fn with_epochs_through(highest: EpochId) -> Arc<dyn GrpcIndexes> {
        let mut entries = HashMap::new();
        for epoch in 0..=highest {
            entries.insert(epoch, fully_populated_entry(epoch));
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

/// A recognizable, non-trivial byte pattern stamped into the test
/// fixture's `start_system_state` so the snapshot round-trip can prove
/// the opaque bytes pass through writer → BCS → file → BCS → reader
/// untouched. Mixed-bit nibbles (`0xA5`, `0x5A`) and a length not aligned
/// to common word sizes (37 bytes) surface off-by-one truncation, byte
/// swap, or fixed-padding bugs as a specific mismatch rather than an
/// all-zero or all-`0xFF` clobber.
const TEST_START_SYSTEM_STATE: &[u8] = &[
    0xA5, 0x5A, 0xDE, 0xAD, 0xBE, 0xEF, 0xC0, 0xFF, 0xEE, 0x42, 0x13, 0x37, 0x00, 0x01, 0x02, 0x03,
    0x04, 0x05, 0x06, 0x07, 0xFE, 0xED, 0xFA, 0xCE, 0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0,
    0xA5, 0x5A, 0xCA, 0xFE, 0x99,
];

fn fully_populated_entry(epoch: EpochId) -> EpochInfoEntry {
    let summary = CheckpointSummary {
        epoch,
        sequence_number: 0,
        network_total_transactions: 0,
        content_digest: Default::default(),
        previous_digest: None,
        epoch_rolling_gas_cost_summary: GasCostSummary::default(),
        end_of_epoch_data: None,
        timestamp_ms: 0,
        version_specific_data: Vec::new(),
        checkpoint_commitments: Vec::new(),
    };
    let sig = AuthorityStrongQuorumSignInfo {
        epoch,
        signature: Default::default(),
        signers_map: Default::default(),
    };
    EpochInfoEntry {
        first_checkpoint: 0,
        start_system_state: TEST_START_SYSTEM_STATE.to_vec(),
        last_checkpoint_summary: Some(Envelope::new_from_data_and_sig(summary, sig)),
        end_of_epoch_tx_events: Some(TransactionEvents::default()),
    }
}

impl GrpcIndexes for TestGrpcIndexes {
    fn get_epoch_info(&self, _epoch: EpochId) -> StorageResult<Option<EpochInfo>> {
        Ok(None)
    }

    fn get_epoch_info_entry(&self, epoch: EpochId) -> StorageResult<Option<EpochInfoEntry>> {
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
        _parent: ObjectID,
        _cursor: Option<ObjectID>,
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
        _original_package_id: ObjectID,
        _cursor: Option<u64>,
    ) -> StorageResult<Box<dyn Iterator<Item = PackageVersionIteratorItem> + '_>> {
        Ok(Box::new(std::iter::empty()))
    }
}

pub fn insert_keys(
    db: &AuthorityPerpetualTables,
    total_unique_object_ids: u64,
) -> Result<(), anyhow::Error> {
    let mut id = ObjectID::ZERO;
    for _ in 0..total_unique_object_ids {
        let object = Object::immutable_with_id_for_testing(id);
        // Use a concrete `Some(0)` so the snapshot writer's V1-rejection
        // check passes. The exact value is irrelevant for the round-trip
        // tests, which compare object references rather than checkpoints.
        db.insert_object_test_only(object, Some(0))?;
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
// TODO(iota-snapshot tests): the two cases (populated, empty) are merged into
// `test_snapshot_round_trip` as a single `#[tokio::test]` to sidestep the
// `typed_store::DBMetrics` global Prometheus registry race documented at
// `crates/typed-store/src/metrics.rs` (`once_cell::sync::OnceCell`-based
// initialization that races between concurrent test threads). Do not split
// these back into separate tests until that registry is made re-entrant.
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
    // followed by `bcs(EpochInfo)`. The reader does not consume this file
    // during restore (the indexer reads it out-of-band from the bucket), so
    // without this assertion a writer bug - typo'd magic, wrong filename,
    // wrong BCS encoding - would pass every test and only surface
    // post-deploy when the indexer fails to decode. Gated on `None`
    // compression so the raw on-disk bytes are readable directly.
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
        let decoded: SnapshotEpochInfo = bcs::from_bytes(&bytes[MAGIC_BYTES..])?;
        let SnapshotEpochInfo::V1(decoded_v1) = decoded;
        // `TestGrpcIndexes::with_epochs_through(0)` provides exactly one
        // populated entry for epoch 0, so the snapshot's EPOCH_INFO must
        // carry exactly one entry.
        assert_eq!(
            decoded_v1.entries.len(),
            1,
            "expected `entries` of length 1 for snapshot at epoch 0"
        );
        // `EpochInfoV1.entries` is `Vec<Option<EpochInfoEntry>>`; the writer
        // always emits `Some(_)` under its `EpochIndexed` watermark
        // precondition (see `StateSnapshotWriterV1::check_epoch_indexed_watermark`).
        let entry = decoded_v1.entries[0]
            .as_ref()
            .expect("writer must emit Some entry under the watermark precondition");
        // Bit-identical round-trip of `start_system_state`. This is the
        // canary for the contract that the snapshot crate treats the inner
        // BCS as opaque bytes (never decodes, never reframes) — a writer
        // bug that truncated, padded, or re-encoded the field would change
        // these bytes and fail here, even though the outer BCS round-trip
        // would still succeed.
        assert_eq!(
            entry.start_system_state.as_slice(),
            TEST_START_SYSTEM_STATE,
            "start_system_state did not round-trip bit-identical through the snapshot"
        );
        assert_eq!(
            entry.first_checkpoint, 0,
            "first_checkpoint did not round-trip"
        );
        assert!(
            entry.last_checkpoint_summary.is_some(),
            "last_checkpoint_summary did not round-trip"
        );
        assert!(
            entry.end_of_epoch_tx_events.is_some(),
            "end_of_epoch_tx_events did not round-trip"
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

#[tokio::test]
async fn test_snapshot_round_trip() -> Result<(), anyhow::Error> {
    // Populated case, with compression - exercises the production path.
    let basic_dir = iota_common::tempdir();
    snapshot_round_trip(basic_dir.path(), 1000, FileCompression::Zstd).await?;
    // Empty database case.
    let empty_dir = iota_common::tempdir();
    snapshot_round_trip(empty_dir.path(), 0, FileCompression::Zstd).await?;
    // Uncompressed case so the ref-file on-disk size assertion can run
    // directly against the staged file.
    let uncompressed_dir = iota_common::tempdir();
    snapshot_round_trip(uncompressed_dir.path(), 100, FileCompression::None).await?;
    // Per-object `previous_transaction_checkpoint` round-trip. Lives in the
    // same `#[tokio::test]` as the other sub-cases to sidestep the
    // `typed_store::DBMetrics` global Prometheus registry race (see comment
    // on `snapshot_round_trip`).
    let checkpoint_dir = iota_common::tempdir();
    snapshot_restores_per_object_checkpoint(checkpoint_dir.path()).await?;

    // V1-rejection: a perpetual DB containing any row whose
    // `previous_transaction_checkpoint` is `None` (i.e. lifted from a pre-V2
    // on-disk format) must cause the writer to error out before any `.obj`
    // file is written. Two sub-cases cover the same boundary check at
    // different layers:
    //   - `lifted_v1_row`: V2 row with `previous_transaction_checkpoint: None`
    //     inserted directly, isolating the writer's rejection check.
    //   - `literal_v1_row`: literal `StoreObjectWrapper::V1` row inserted, forcing
    //     `LiveSetIter::migrate()` to lift the row to V2(None) before the writer
    //     sees it. Locks the end-to-end pipeline as a single assertion.
    let lifted_v1_dir = iota_common::tempdir();
    snapshot_writer_rejects_lifted_v1_row(lifted_v1_dir.path()).await?;
    let literal_v1_dir = iota_common::tempdir();
    snapshot_writer_rejects_literal_v1_row(literal_v1_dir.path()).await?;

    // Watermark precondition: absent watermark rejects the snapshot.
    // Matched against the full anyhow chain via `{err:#}` because
    // `write_internal` wraps the inner error with a context message.
    let watermark_absent_dir = iota_common::tempdir();
    let err =
        writer_with_stub_returns_err(watermark_absent_dir.path(), 0, TestGrpcIndexes::empty())
            .await;
    let msg = format!("{err:#}");
    assert!(
        msg.contains("`EpochIndexed` watermark is absent"),
        "absent-watermark error chain did not match: {msg}"
    );

    // Watermark precondition: `Some(h)` with `h < snapshot_epoch` rejects
    // the snapshot. The "watermark is at epoch N, but snapshot_epoch is M"
    // wording is itself part of the operator-facing contract — pin both
    // sides so a rewording that drops one is caught here.
    let watermark_below_dir = iota_common::tempdir();
    let err = writer_with_stub_returns_err(
        watermark_below_dir.path(),
        5,
        TestGrpcIndexes::watermark_only(3),
    )
    .await;
    let msg = format!("{err:#}");
    assert!(
        msg.contains("`EpochIndexed` watermark is at epoch 3"),
        "too-low watermark error chain did not match (epoch 3): {msg}"
    );
    assert!(
        msg.contains("snapshot_epoch is 5"),
        "too-low watermark error chain did not match (snapshot_epoch 5): {msg}"
    );

    Ok(())
}

/// Asserts that per-object `previous_transaction_checkpoint` survives the
/// snapshot round-trip end-to-end: stamped on `StoreObjectV2` at insert
/// time, surfaced by `LiveSetIter`, BCS-encoded into `LiveObject` records
/// in the `.obj` files, decoded by `LiveObjectIter`, and re-stamped onto
/// the restored DB by `bulk_insert_live_objects`. Without this, a
/// regression that, e.g., reverted the restore path to stamping `None`
/// would still pass `snapshot_round_trip` (which only compares
/// `object_reference`s) - this helper is the focused canary for the
/// per-object checkpoint contract.
async fn snapshot_restores_per_object_checkpoint(
    tmp_dir: &std::path::Path,
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
    let mut expected: HashMap<ObjectID, Option<u64>> = HashMap::new();
    let mut id = ObjectID::ZERO;
    for i in 0..NUM_OBJECTS {
        let object = Object::immutable_with_id_for_testing(id);
        let checkpoint = CHECKPOINT_BASE | i;
        perpetual_db.insert_object_test_only(object, Some(checkpoint))?;
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
    let restored: HashMap<ObjectID, Option<u64>> = restored_perpetual_db
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
/// synced from genesis under V2 sees the problem at publish time, not after the
/// bad snapshot has been uploaded.
async fn snapshot_writer_rejects_lifted_v1_row(
    tmp_dir: &std::path::Path,
) -> Result<(), anyhow::Error> {
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
        FileCompression::None,
        NonZeroUsize::new(1).unwrap(),
    )
    .await?;

    let perpetual_db = Arc::new(AuthorityPerpetualTables::open(&tmp_dir.join("db"), None));
    // Mix a normal V2 row (Some(seq)) with a lifted-V1 row (None) so the
    // assertion is genuinely about the lifted row triggering the error,
    // not about the DB being empty.
    perpetual_db.insert_object_test_only(
        Object::immutable_with_id_for_testing(ObjectID::ZERO),
        Some(42),
    )?;
    let lifted_id = ObjectID::ZERO.next_lexicographical();
    perpetual_db.insert_object_test_only(Object::immutable_with_id_for_testing(lifted_id), None)?;

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

/// End-to-end variant of [`snapshot_writer_rejects_lifted_v1_row`]: inserts
/// a literal `StoreObjectWrapper::V1` row directly into the perpetual
/// `objects` map (bypassing `get_store_object`, which always produces V2),
/// then runs the snapshot writer. The on-read `migrate()` step must lift
/// the row to `StoreObjectV2(None)`, `LiveSetIter` must surface the `None`,
/// and the writer must reject the publish — covering the full V1->lift->reject
/// pipeline as a single assertion.
async fn snapshot_writer_rejects_literal_v1_row(
    tmp_dir: &std::path::Path,
) -> Result<(), anyhow::Error> {
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
        FileCompression::None,
        NonZeroUsize::new(1).unwrap(),
    )
    .await?;

    let perpetual_db = Arc::new(AuthorityPerpetualTables::open(&tmp_dir.join("db"), None));
    // Single literal V1 row. The writer must fail end-to-end.
    perpetual_db
        .insert_v1_object_test_only(Object::immutable_with_id_for_testing(ObjectID::ZERO))?;

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
    let manifest = Manifest::V2(ManifestBody {
        snapshot_version: 2,
        address_length: ObjectID::LENGTH as u64,
        file_metadata: vec![FileMetadata {
            file_type: FileType::Reference,
            bucket_num: 1,
            part_num: 1,
            file_compression: FileCompression::None,
            sha3_digest: [0u8; 32],
        }],
        epoch: 0,
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
/// with its length preserved.
///
/// The end-to-end payload (writer → BCS → file → BCS → reader) is covered
/// by `snapshot_round_trip`'s EPOCH_INFO assertion, which uses the
/// `TEST_START_SYSTEM_STATE` canary to verify a bit-identical round-trip
/// of the opaque `start_system_state` bytes plus `first_checkpoint`,
/// `last_checkpoint_summary`, and `end_of_epoch_tx_events`. This test
/// exercises the discriminant and Vec framing in isolation so an on-disk-
/// format regression that decouples from the writer (e.g. someone adds
/// an `EpochInfoV2` variant before `V1`) is caught even if the writer
/// path is healthy.
#[test]
fn epoch_info_v1_bcs_round_trip() {
    // Mix of `Some(_)` and `None` entries so the `Option` framing in
    // `Vec<Option<EpochInfoEntry>>` is exercised end-to-end. Today's
    // writer never emits `None` (the watermark precondition forbids it),
    // but the on-disk format reserves that shape for a future partial-
    // coverage writer — see [`EpochInfo`]. Locking the `None`-tag byte
    // here keeps that forward-compat path honest.
    let epoch_info = SnapshotEpochInfo::V1(EpochInfoV1 {
        entries: vec![
            Some(fully_populated_entry(0)),
            None,
            Some(fully_populated_entry(2)),
        ],
    });
    let bytes = bcs::to_bytes(&epoch_info).unwrap();
    assert_eq!(
        bytes[0], 0,
        "EpochInfo::V1 must remain at BCS discriminant 0"
    );

    let decoded: SnapshotEpochInfo = bcs::from_bytes(&bytes).unwrap();
    let SnapshotEpochInfo::V1(decoded_v1) = decoded;
    assert_eq!(decoded_v1.entries.len(), 3);
    assert!(
        decoded_v1.entries[1].is_none(),
        "Vec<Option<_>> framing must preserve None entries across BCS"
    );
    // Per-entry summary carries the epoch number, so this also asserts
    // the Vec ordering is preserved across BCS round-trip.
    let assert_entry_epoch = |i: usize, expected: EpochId| {
        let entry = decoded_v1.entries[i]
            .as_ref()
            .expect("fixture populates this slot");
        let summary = entry
            .last_checkpoint_summary
            .as_ref()
            .expect("fixture populates last_checkpoint_summary");
        assert_eq!(summary.epoch(), expected);
    };
    assert_entry_epoch(0, 0);
    assert_entry_epoch(2, 2);
}
