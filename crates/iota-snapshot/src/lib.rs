// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#![allow(dead_code)]

#[cfg(test)]
mod tests;

pub mod reader;
pub mod restore;
pub mod uploader;
mod writer;

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use anyhow::Result;
use fastcrypto::hash::MultisetHash;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use iota_core::{
    authority::{
        authority_store_tables::{AuthorityPerpetualTables, LiveObject},
        epoch_start_configuration::{EpochFlag, EpochStartConfiguration},
    },
    checkpoints::CheckpointStore,
    epoch::committee_store::CommitteeStore,
    global_state_hasher::GlobalStateHasher,
};
use iota_storage::{
    FileCompression, SHA3_BYTES, compute_sha3_checksum, object_store::util::path_to_filesystem,
};
use iota_types::{
    base_types::ObjectID,
    epoch_info::EpochInfoEntry,
    global_state_hash::GlobalStateHash,
    iota_system_state::{
        IotaSystemStateTrait, epoch_start_iota_system_state::EpochStartSystemStateTrait,
        get_iota_system_state,
    },
    messages_checkpoint::ECMHLiveObjectSetDigest,
};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use object_store::path::Path;
use serde::{Deserialize, Serialize};
use tokio::time::Instant;

/// The following describes the on-disk format of a snapshot, as written by
/// `StateSnapshotWriterV1` and consumed by `StateSnapshotReaderV1`. The
/// snapshot is taken at the end of every epoch and stores the live object
/// set bucketed by the same hash function used for the global state hash
/// accumulator, allowing a single bucket to be consumed in parallel. Each
/// bucket is split across one or more partitions; a partition is a single
/// `.obj` file with a maximum size of 128 MB and a matching `.ref` file
/// listing the object references in that partition. Partition files are
/// optionally zstd-compressed.
///
/// V2 additions over V1:
/// - REFERENCE file magic is `0xCAFEBEEF` (V1 was `0xDEADBEEF`); a V2 reader
///   fails fast on a V1 magic and vice versa.
/// - REFERENCE records carry an extra 8-byte big-endian
///   `previous_transaction_checkpoint` trailer per record (80-byte records
///   instead of V1's 72).
/// - A per-snapshot `EPOCH_INFO` file is emitted alongside the bucket files,
///   carrying one entry per epoch in `[0, snapshot_epoch]` from
///   `CheckpointStore::epoch_info`.
///
/// State Snapshot Directory Layout
///  - snapshot/
///     - epoch_0/
///        - 1_1.obj
///        - 1_2.obj
///        - 1_3.obj
///        - 2_1.obj
///        - ...
///        - 1_1.ref
///        - 1_2.ref
///        - 2_1.ref
///        - ...
///        - EPOCH_INFO
///        - MANIFEST
///     - epoch_1/
///       - 1_1.obj
///       - ...
///
/// Object File Disk Format
/// ┌──────────────────────────────┐
/// │  magic(0x00B7EC75) <4 byte>  │
/// ├──────────────────────────────┤
/// │ ┌──────────────────────────┐ │
/// │ │         Object 1         │ │
/// │ ├──────────────────────────┤ │
/// │ │          ...             │ │
/// │ ├──────────────────────────┤ │
/// │ │         Object N         │ │
/// │ └──────────────────────────┘ │
/// └──────────────────────────────┘
/// Object
/// ┌───────────────┬───────────────────┬──────────────┐
/// │ len <uvarint> │ encoding <1 byte> │ data <bytes> │
/// └───────────────┴───────────────────┴──────────────┘
///
/// REFERENCE File Disk Format (V2)
/// ┌──────────────────────────────┐
/// │  magic(0xCAFEBEEF) <4 byte>  │
/// ├──────────────────────────────┤
/// │ ┌──────────────────────────┐ │
/// │ │       ObjectRefV2 1      │ │
/// │ ├──────────────────────────┤ │
/// │ │          ...             │ │
/// │ ├──────────────────────────┤ │
/// │ │       ObjectRefV2 N      │ │
/// │ └──────────────────────────┘ │
/// └──────────────────────────────┘
/// ObjectRefV2: 80 bytes total, fields concatenated in declaration order:
///   - ObjectID                          : 32 bytes
///   - SequenceNumber                    :  8 bytes
///   - ObjectDigest                      : 32 bytes
///   - previous_transaction_checkpoint   :  8 bytes (big-endian u64)
///
/// EPOCH_INFO File Disk Format
/// ┌──────────────────────────────┐
/// │  magic(0x9000C001) <4 byte>  │
/// ├──────────────────────────────┤
/// │   bcs(EpochInfo)             │
/// └──────────────────────────────┘
/// Integrity is anchored by `FileMetadata::sha3_digest` recorded in the
/// MANIFEST (matching how `.obj`/`.ref` files are validated); no in-file
/// sha3 trailer is written.
///
/// MANIFEST File Disk Format
/// ┌──────────────────────────────┐
/// │  magic(0x00C0FFEE) <4 byte>  │
/// ├──────────────────────────────┤
/// │   serialized manifest        │
/// ├──────────────────────────────┤
/// │      sha3 <32 bytes>         │
/// └──────────────────────────────┘
const OBJECT_FILE_MAGIC: u32 = 0x00B7EC75;
/// Magic for V2 reference files. Distinct from the V1 magic (`0xDEADBEEF`) so
/// a V1 reader fails fast on the magic check rather than silently
/// mis-decoding a V2 ref record's extra `previous_transaction_checkpoint`
/// trailer.
const REFERENCE_FILE_MAGIC_V2: u32 = 0xCAFEBEEF;
const EPOCH_INFO_FILE_MAGIC: u32 = 0x9000C001;
const MANIFEST_FILE_MAGIC: u32 = 0x00C0FFEE;
const MAGIC_BYTES: usize = 4;
const SNAPSHOT_VERSION_BYTES: usize = 1;
const ADDRESS_LENGTH_BYTES: usize = 8;
const PADDING_BYTES: usize = 3;
const MANIFEST_FILE_HEADER_BYTES: usize =
    MAGIC_BYTES + SNAPSHOT_VERSION_BYTES + ADDRESS_LENGTH_BYTES + PADDING_BYTES;
const FILE_MAX_BYTES: usize = 128 * 1024 * 1024;
const OBJECT_ID_BYTES: usize = ObjectID::LENGTH;
const SEQUENCE_NUM_BYTES: usize = 8;
const OBJECT_DIGEST_BYTES: usize = 32;
/// Size of a V2 reference record: 72-byte V1 ObjectRef (ObjectID +
/// SequenceNumber + ObjectDigest) plus an 8-byte big-endian
/// `previous_transaction_checkpoint` trailer.
const PREV_TX_CHECKPOINT_BYTES: usize = 8;
const OBJECT_REF_BYTES_V2: usize =
    OBJECT_ID_BYTES + SEQUENCE_NUM_BYTES + OBJECT_DIGEST_BYTES + PREV_TX_CHECKPOINT_BYTES;
const FILE_TYPE_BYTES: usize = 1;
const BUCKET_BYTES: usize = 4;
const BUCKET_PARTITION_BYTES: usize = 4;
const COMPRESSION_TYPE_BYTES: usize = 1;
const FILE_METADATA_BYTES: usize =
    FILE_TYPE_BYTES + BUCKET_BYTES + BUCKET_PARTITION_BYTES + COMPRESSION_TYPE_BYTES + SHA3_BYTES;

#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TryFromPrimitive, IntoPrimitive,
)]
#[repr(u8)]
pub enum FileType {
    Object = 0,
    Reference = 1,
    /// V2 only: per-epoch metadata file, populated from `CheckpointStore`'s
    /// `epoch_info` table.
    EpochInfo = 2,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
/// FileMetadata holds either an object or a reference file metadata.
pub struct FileMetadata {
    pub file_type: FileType,
    pub bucket_num: u32,
    pub part_num: u32,
    pub file_compression: FileCompression,
    pub sha3_digest: [u8; 32],
}

impl FileMetadata {
    pub fn file_path(&self, dir_path: &Path) -> Path {
        match self.file_type {
            FileType::Object => {
                dir_path.child(&*format!("{}_{}.obj", self.bucket_num, self.part_num))
            }
            FileType::Reference => {
                dir_path.child(&*format!("{}_{}.ref", self.bucket_num, self.part_num))
            }
            // EPOCH_INFO is a singleton per snapshot, so bucket/part numbers
            // do not contribute to the filename.
            FileType::EpochInfo => dir_path.child("EPOCH_INFO"),
        }
    }
    pub fn local_file_path(&self, root_path: &std::path::Path, dir_path: &Path) -> Result<PathBuf> {
        path_to_filesystem(root_path.to_path_buf(), &self.file_path(dir_path))
    }
}

/// Body of a manifest at any version. V1 and V2 are structurally identical —
/// the on-disk wire format is the same and the BCS variant tag on `Manifest`
/// distinguishes them. V2 differs only in semantic associations: the
/// `file_metadata` list includes the per-snapshot `EPOCH_INFO` file, and
/// `.ref` files carry 80-byte records (with a `previous_transaction_checkpoint`
/// trailer). `address_length` is preserved as a sanity check across versions.
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct ManifestBody {
    pub snapshot_version: u8,
    pub address_length: u64,
    pub file_metadata: Vec<FileMetadata>,
    pub epoch: u64,
}

// `Manifest::V1` and `Manifest::V2` use the same `ManifestBody` payload —
// the BCS variant tag distinguishes them. The variants must stay (removing
// `V1` would shift `V2`'s tag) even though the body type is shared.
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum Manifest {
    V1(ManifestBody),
    V2(ManifestBody),
}

impl Manifest {
    fn body(&self) -> &ManifestBody {
        match self {
            Self::V1(manifest) | Self::V2(manifest) => manifest,
        }
    }
    pub fn snapshot_version(&self) -> u8 {
        self.body().snapshot_version
    }
    pub fn address_length(&self) -> u64 {
        self.body().address_length
    }
    pub fn file_metadata(&self) -> &Vec<FileMetadata> {
        &self.body().file_metadata
    }
    pub fn epoch(&self) -> u64 {
        self.body().epoch
    }
}

/// On-disk schema for the per-snapshot `EPOCH_INFO` file. Versioned to allow
/// future schema evolution. `entries[i]` is the entry for epoch `i`; `None`
/// indicates the source `epoch_info` table had no row for that epoch.
/// Length is `snapshot_epoch + 1`.
// Note: no `Eq`/`PartialEq` derive here. `EpochInfoEntry` transitively
// contains `BLS12381AggregateSignature`, which does not implement `PartialEq`.
#[derive(Debug, Serialize, Deserialize)]
pub enum EpochInfo {
    V1(EpochInfoV1),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EpochInfoV1 {
    pub entries: Vec<Option<EpochInfoEntry>>,
}

impl EpochInfo {
    pub fn entries(&self) -> &[Option<EpochInfoEntry>] {
        match self {
            Self::V1(info) => &info.entries,
        }
    }
}

/// Creates a FileMetadata of the provided file path, which is overwritten with
/// compressed data of the original file.
pub fn create_file_metadata(
    file_path: &std::path::Path,
    file_compression: FileCompression,
    file_type: FileType,
    bucket_num: u32,
    part_num: u32,
) -> Result<FileMetadata> {
    // Overwrites the file with compressed data of the original file.
    file_compression.compress(file_path)?;
    // Computes the sha3 checksum of the compressed file.
    let sha3_digest = compute_sha3_checksum(file_path)?;
    let file_metadata = FileMetadata {
        file_type,
        bucket_num,
        part_num,
        file_compression,
        sha3_digest,
    };
    Ok(file_metadata)
}

pub async fn setup_db_state(
    epoch: u64,
    state_hash: GlobalStateHash,
    perpetual_db: Arc<AuthorityPerpetualTables>,
    checkpoint_store: Arc<CheckpointStore>,
    committee_store: Arc<CommitteeStore>,
    verify: bool,
    num_live_objects: u64,
    m: MultiProgress,
) -> Result<()> {
    // This function should be called once state accumulator based hash verification
    // is complete and live object set state is downloaded to local store
    let system_state_object = get_iota_system_state(&perpetual_db)?;
    let new_epoch_start_state = system_state_object.into_epoch_start_state();
    let next_epoch_committee = new_epoch_start_state.get_iota_committee();
    let root_digest: ECMHLiveObjectSetDigest = state_hash.digest().into();
    let last_checkpoint = checkpoint_store
        .get_epoch_last_checkpoint(epoch)
        .expect("Error loading last checkpoint for current epoch")
        .expect("Could not load last checkpoint for current epoch");
    let flags = EpochFlag::default_for_no_config();
    let epoch_start_configuration = EpochStartConfiguration::new(
        new_epoch_start_state,
        *last_checkpoint.digest(),
        &perpetual_db,
        flags,
    )
    .unwrap();
    perpetual_db.set_epoch_start_configuration(&epoch_start_configuration)?;
    perpetual_db.insert_root_state_hash(epoch, last_checkpoint.sequence_number, state_hash)?;
    perpetual_db.set_highest_pruned_checkpoint_without_wb(last_checkpoint.sequence_number)?;
    committee_store.insert_new_committee(&next_epoch_committee)?;
    checkpoint_store.update_highest_executed_checkpoint(&last_checkpoint)?;

    if verify {
        let iter = perpetual_db.iter_live_object_set();
        let local_digest = ECMHLiveObjectSetDigest::from(
            accumulate_live_object_iter(Box::new(iter), m.clone(), num_live_objects)
                .await
                .digest(),
        );
        assert_eq!(
            root_digest, local_digest,
            "End of epoch {} root state digest {} does not match \
                local root state hash {} after restoring db from formal snapshot",
            epoch, root_digest.digest, local_digest.digest,
        );
        println!("DB live object state verification completed successfully!");
    }

    Ok(())
}

pub async fn accumulate_live_object_iter(
    iter: Box<dyn Iterator<Item = LiveObject> + '_>,
    m: MultiProgress,
    num_live_objects: u64,
) -> GlobalStateHash {
    // Monitor progress of live object accumulation
    let accum_progress_bar = m.add(ProgressBar::new(num_live_objects).with_style(
        ProgressStyle::with_template("[{elapsed_precise}] {wide_bar} {pos}/{len} ({msg})").unwrap(),
    ));
    let accum_counter = Arc::new(AtomicU64::new(0));
    let cloned_accum_counter = accum_counter.clone();
    let cloned_progress_bar = accum_progress_bar.clone();
    let handle = tokio::spawn(async move {
        let a_instant = Instant::now();
        loop {
            if cloned_progress_bar.is_finished() {
                break;
            }
            let num_accumulated = cloned_accum_counter.load(Ordering::Relaxed);
            assert!(
                num_accumulated <= num_live_objects,
                "Accumulated more objects (at least {num_accumulated}) than expected ({num_live_objects})"
            );
            let accumulations_per_sec = num_accumulated as f64 / a_instant.elapsed().as_secs_f64();
            cloned_progress_bar.set_position(num_accumulated);
            cloned_progress_bar.set_message(format!(
                "DB live obj accumulations per sec: {accumulations_per_sec}"
            ));
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    // Accumulate live objects
    let mut acc = GlobalStateHash::default();
    for live_object in iter {
        GlobalStateHasher::accumulate_live_object(&mut acc, &live_object);
        accum_counter.fetch_add(1, Ordering::Relaxed);
    }
    accum_progress_bar.finish_with_message("DB live object accumulation completed");
    handle
        .await
        .expect("Failed to join live object accumulation progress monitor");
    acc
}
