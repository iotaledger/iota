// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashSet, fs, io::Write, num::NonZeroUsize, sync::Arc};

use byteorder::{BigEndian, ByteOrder};
use fastcrypto::hash::{HashFunction, MultisetHash, Sha3_256};
use futures::future::AbortHandle;
use indicatif::MultiProgress;
use iota_config::object_storage_config::{ObjectStoreConfig, ObjectStoreType};
use iota_core::{
    authority::authority_store_tables::AuthorityPerpetualTables, checkpoints::CheckpointStore,
    global_state_hasher::GlobalStateHasher,
};
use iota_types::{
    base_types::ObjectID, global_state_hash::GlobalStateHash,
    messages_checkpoint::ECMHLiveObjectSetDigest, object::Object,
};

use crate::{
    EPOCH_INFO_FILE_MAGIC, EpochInfo, EpochInfoV1, FileCompression, FileMetadata, FileType,
    MAGIC_BYTES, MANIFEST_FILE_MAGIC, Manifest, ManifestBody, OBJECT_REF_BYTES_V2,
    reader::StateSnapshotReaderV1, writer::StateSnapshotWriterV1,
};

pub fn insert_keys(
    db: &AuthorityPerpetualTables,
    total_unique_object_ids: u64,
) -> Result<(), anyhow::Error> {
    let mut id = ObjectID::ZERO;
    for _ in 0..total_unique_object_ids {
        let object = Object::immutable_with_id_for_testing(id);
        db.insert_object_test_only(object)?;
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
        file_compression,
        NonZeroUsize::new(1).unwrap(),
    )
    .await?;
    let perpetual_db = Arc::new(AuthorityPerpetualTables::open(&tmp_dir.join("db"), None));
    insert_keys(&perpetual_db, num_objects)?;
    let root_accumulator =
        ECMHLiveObjectSetDigest::from(accumulate_live_object_set(&perpetual_db).digest());
    let checkpoint_store = CheckpointStore::new(&tmp_dir.join("checkpoint_store"));
    snapshot_writer
        .write_internal(0, perpetual_db.clone(), checkpoint_store, root_accumulator)
        .await?;

    // On-wire size assertion: with no compression the uploaded `.ref` file
    // is exactly `MAGIC_BYTES + num_objects * OBJECT_REF_BYTES_V2`. This
    // locks the V2 trailer width - a bug that wrote records of the wrong
    // size would still pass the round-trip if writer and reader agreed on
    // the wrong size. Reads from the remote store, since `sync_file_to_remote`
    // removes the local copy after upload.
    if file_compression == FileCompression::None && num_objects > 0 {
        let ref_file = tmp_dir.join("remote_dir").join("epoch_0").join("1_1.ref");
        let actual_size = fs::metadata(&ref_file)?.len() as usize;
        let expected_size = MAGIC_BYTES + (num_objects as usize) * OBJECT_REF_BYTES_V2;
        assert_eq!(
            actual_size, expected_size,
            "ref-file on-wire size mismatch: expected {expected_size}, got {actual_size}"
        );
    }

    // Lock the EPOCH_INFO file's on-disk shape: 4-byte big-endian magic
    // followed by `bcs(EpochInfo)`. The reader does not consume this file
    // during restore (the indexer reads it out-of-band from the bucket), so
    // without this assertion a writer bug - typo'd magic, wrong filename,
    // wrong BCS encoding - would pass every test and only surface
    // post-deploy when the indexer fails to decode. Gated on `None`
    // compression so the raw on-wire bytes are readable directly.
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
        // The round-trip test uses an empty `CheckpointStore`, so for
        // epoch 0 the writer emits a single `None` entry.
        assert_eq!(
            decoded_v1.entries.len(),
            1,
            "expected `entries` of length 1 for snapshot at epoch 0"
        );
        assert!(
            decoded_v1.entries[0].is_none(),
            "expected `entries[0]` to be `None` (CheckpointStore is empty)"
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

#[tokio::test]
async fn test_snapshot_round_trip() -> Result<(), anyhow::Error> {
    // Populated case, with compression - exercises the production path.
    let basic_dir = iota_common::tempdir();
    snapshot_round_trip(basic_dir.path(), 1000, FileCompression::Zstd).await?;
    // Empty database case.
    let empty_dir = iota_common::tempdir();
    snapshot_round_trip(empty_dir.path(), 0, FileCompression::Zstd).await?;
    // Uncompressed case so the ref-file on-wire size assertion can run
    // directly against the staged file.
    let uncompressed_dir = iota_common::tempdir();
    snapshot_round_trip(uncompressed_dir.path(), 100, FileCompression::None).await?;
    Ok(())
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

/// Locks the on-wire format of the `EPOCH_INFO` file body: BCS-encoding
/// `EpochInfo::V1` must use variant tag `0`, and `entries` must round-trip
/// with its length and `None`/`Some` shape preserved. `EpochInfoEntry`
/// does not implement `PartialEq`, so this covers the all-`None` shape
/// end-to-end. The `Some(..)` payload is not exercised here because
/// constructing a real `CertifiedCheckpointSummary` requires committee
/// fixtures from `iota-swarm-config`, which this crate does not depend on.
#[test]
fn epoch_info_v1_bcs_round_trip() {
    let epoch_info = EpochInfo::V1(EpochInfoV1 {
        entries: vec![None, None, None],
    });
    let bytes = bcs::to_bytes(&epoch_info).unwrap();
    assert_eq!(
        bytes[0], 0,
        "EpochInfo::V1 must remain at BCS discriminant 0"
    );

    let decoded: EpochInfo = bcs::from_bytes(&bytes).unwrap();
    let EpochInfo::V1(decoded_v1) = decoded;
    assert_eq!(decoded_v1.entries.len(), 3);
    assert!(decoded_v1.entries.iter().all(Option::is_none));
}
