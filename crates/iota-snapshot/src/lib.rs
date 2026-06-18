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
    collections::HashSet,
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
use iota_sdk_types::ObjectId;
use iota_storage::{
    FileCompression, SHA3_BYTES, compute_sha3_checksum, object_store::util::path_to_filesystem,
};
use iota_types::{
    IOTA_SYSTEM_STATE_OBJECT_ID,
    base_types::ObjectRef,
    committee::{Committee, CommitteeChainVerifier},
    digests::ChainIdentifier,
    effects::{TransactionEffectsAPI, TransactionEffectsExt},
    global_state_hash::GlobalStateHash,
    iota_system_state::{
        IotaSystemState, IotaSystemStateTrait,
        epoch_start_iota_system_state::EpochStartSystemStateTrait, get_iota_system_state,
    },
    messages_checkpoint::{CheckpointSequenceNumber, ECMHLiveObjectSetDigest},
    object::Object,
    storage::{EpochInfoV1Entry, EpochInfoV2},
};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use object_store::path::Path;
use serde::{Deserialize, Serialize};
use tokio::time::Instant;

use crate::restore::RestoreEpochInfo;

/// The following describes the format of an object file (*.obj) used for
/// persisting live iota objects. The maximum size per .obj file is 128MB. State
/// snapshot will be taken at the end of every epoch. Live object set is split
/// into and stored across multiple hash buckets. The hashing function used
/// for bucketing objects is the same as the one used to build the accumulator
/// tree for computing state root hash. Buckets are further subdivided into
/// partitions. A partition is a smallest storage unit which holds a subset of
/// objects in one bucket. Each partition is a single *.obj file where
/// objects are appended to in an append-only fashion. A new partition is
/// created when the current one reaches its maximum size. i.e. 128MB.
/// Partitions allow a single hash bucket to be consumed in parallel. Partition
/// files are optionally compressed with the zstd compression format. Partition
/// filenames follows the format <bucket_number>_<partition_number>.obj. Object
/// references for hash. There is one single ref file per hash bucket. Object
/// references are written in an append-only manner as well. Finally, the
/// MANIFEST file contains per file metadata of every file in the snapshot
/// directory.
///
/// Snapshot-format V2 additions over V1:
/// - OBJECT file magic is `0x00B7EC76` (V1 was `0x00B7EC75`); a V2 reader fails
///   fast on a V1 magic and vice versa. Encoded records are BCS-serialized
///   `SnapshotLiveObject` carrying the per-object
///   `previous_transaction_checkpoint` inline. The writer rejects rows whose
///   checkpoint is `None` (lifted from pre-V2 store rows) at the publish
///   boundary, so any record present in a published `.obj` file carries a
///   concrete checkpoint sequence number.
/// - REFERENCE file format is unchanged from V1.
/// - A per-snapshot `EPOCH_INFO` file is emitted alongside the bucket files,
///   carrying one [`EpochInfoV1Entry`] per epoch in `[0, snapshot_epoch]` from
///   `IndexStoreTables::epoch_info`. Writer-node operator contract:
///   `enable_grpc_api = true`; the writer refuses to publish unless
///   `Watermark::EpochIndexed >= snapshot_epoch`.
/// - `MANIFEST` is now [`ManifestV2`], adding a `chain_id` field so a restore
///   can reject a foreign-chain snapshot.
///
/// State Snapshot Directory Layout
///  - snapshot/
///     - epoch_0/
///        - 1_1.obj
///        - 1_2.obj
///        - 1_3.obj
///        - 2_1.obj
///        - ...
///        - 1000_1.obj
///        - REFERENCE-1
///        - REFERENCE-2
///        - ...
///        - REFERENCE-1000
///        - EPOCH_INFO
///        - MANIFEST
///     - epoch_1/
///       - 1_1.obj
///       - ...
///
/// Object File Disk Format
/// ┌──────────────────────────────┐
/// │  magic(0x00B7EC76) <4 byte>  │
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
/// REFERENCE File Disk Format
/// ┌──────────────────────────────┐
/// │  magic(0xDEADBEEF) <4 byte>  │
/// ├──────────────────────────────┤
/// │ ┌──────────────────────────┐ │
/// │ │         ObjectRef 1      │ │
/// │ ├──────────────────────────┤ │
/// │ │          ...             │ │
/// │ ├──────────────────────────┤ │
/// │ │         ObjectRef N      │ │
/// │ └──────────────────────────┘ │
/// └──────────────────────────────┘
/// ObjectRef (ObjectId, SequenceNumber, ObjectDigest)
/// ┌───────────────┬───────────────────┬──────────────┐
/// │         data (<(address_len + 8 + 32) bytes>)    │
/// └───────────────┴───────────────────┴──────────────┘
///
/// EPOCH_INFO File Disk Format
/// ┌──────────────────────────────┐
/// │  magic(0x9000C001) <4 byte>  │
/// ├──────────────────────────────┤
/// │   bcs(EpochInfo)             │
/// └──────────────────────────────┘
/// See [`EpochInfo`] for the schema. `FileMetadata::sha3_digest` in the
/// MANIFEST can be used to verify file integrity.
///
/// MANIFEST File Disk Format
/// ┌──────────────────────────────┐
/// │  magic(0x00C0FFEE) <4 byte>  │
/// ├──────────────────────────────┤
/// │   serialized manifest        │
/// ├──────────────────────────────┤
/// │      sha3 <32 bytes>         │
/// └──────────────────────────────┘
const OBJECT_FILE_MAGIC: u32 = 0x00B7EC76;
const REFERENCE_FILE_MAGIC: u32 = 0xDEADBEEF;
const EPOCH_INFO_FILE_MAGIC: u32 = 0x9000C001;
const MANIFEST_FILE_MAGIC: u32 = 0x00C0FFEE;
const MAGIC_BYTES: usize = 4;
const SNAPSHOT_VERSION_BYTES: usize = 1;
const ADDRESS_LENGTH_BYTES: usize = 8;
const PADDING_BYTES: usize = 3;
const MANIFEST_FILE_HEADER_BYTES: usize =
    MAGIC_BYTES + SNAPSHOT_VERSION_BYTES + ADDRESS_LENGTH_BYTES + PADDING_BYTES;
const FILE_MAX_BYTES: usize = 128 * 1024 * 1024;
const OBJECT_ID_BYTES: usize = ObjectId::LENGTH;
const SEQUENCE_NUM_BYTES: usize = 8;
const OBJECT_DIGEST_BYTES: usize = 32;
const OBJECT_REF_BYTES: usize = OBJECT_ID_BYTES + SEQUENCE_NUM_BYTES + OBJECT_DIGEST_BYTES;
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
    /// per-epoch metadata file
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

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct ManifestV1 {
    pub snapshot_version: u8,
    pub address_length: u64,
    pub file_metadata: Vec<FileMetadata>,
    pub epoch: u64,
}

/// `ManifestV1` plus `chain_id`, letting a restore reject a foreign-chain
/// snapshot.
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct ManifestV2 {
    pub snapshot_version: u8,
    pub address_length: u64,
    pub file_metadata: Vec<FileMetadata>,
    pub epoch: u64,
    pub chain_id: ChainIdentifier,
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum Manifest {
    V1(ManifestV1),
    V2(ManifestV2),
}

impl Manifest {
    pub fn snapshot_version(&self) -> u8 {
        match self {
            Self::V1(manifest) => manifest.snapshot_version,
            Self::V2(manifest) => manifest.snapshot_version,
        }
    }
    pub fn address_length(&self) -> u64 {
        match self {
            Self::V1(manifest) => manifest.address_length,
            Self::V2(manifest) => manifest.address_length,
        }
    }
    pub fn file_metadata(&self) -> &Vec<FileMetadata> {
        match self {
            Self::V1(manifest) => &manifest.file_metadata,
            Self::V2(manifest) => &manifest.file_metadata,
        }
    }
    pub fn epoch(&self) -> u64 {
        match self {
            Self::V1(manifest) => manifest.epoch,
            Self::V2(manifest) => manifest.epoch,
        }
    }
    /// Producing chain's identifier; `None` for V1.
    pub fn chain_id(&self) -> Option<ChainIdentifier> {
        match self {
            Self::V1(_) => None,
            Self::V2(manifest) => Some(manifest.chain_id),
        }
    }
}

/// On-disk schema for the per-snapshot `EPOCH_INFO` file. Versioned for
/// future schema evolution. `entries[i]` is the entry for epoch `i`;
/// length is `snapshot_epoch + 1`.
#[derive(Debug, Serialize, Deserialize)]
pub enum EpochInfo {
    V1(EpochInfoV1),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EpochInfoV1 {
    pub entries: Vec<EpochInfoV1Entry>,
}

impl EpochInfo {
    pub fn entries(&self) -> &[EpochInfoV1Entry] {
        match self {
            Self::V1(info) => &info.entries,
        }
    }

    fn into_entries(self) -> Vec<EpochInfoV1Entry> {
        match self {
            Self::V1(info) => info.entries,
        }
    }
}

/// Chain-verified `EPOCH_INFO`: the `chain_id` matched and every entry was
/// anchored to its certified `last_checkpoint_summary` — the committee chain
/// walked from the genesis committee, every byte hash-checked back to that
/// signed summary. The only constructor is `verify_epoch_info_chain`, so
/// holding one is proof the data is anchored to the operator-provided genesis.
#[derive(Debug)]
pub struct VerifiedEpochInfo {
    epoch_info: EpochInfo,
    committees: Vec<Committee>,
    /// Digest-verified start state per epoch: `[i]` is epoch `i`'s start state
    /// — the genesis root for `i == 0`, else derived from epoch `i - 1`'s
    /// boundary. Length `snapshot_epoch + 2`; the trailing entry has no epoch
    /// info row.
    start_system_states: Vec<IotaSystemState>,
}

impl VerifiedEpochInfo {
    /// Entries for epochs `[0, snapshot_epoch]`, in epoch order.
    pub fn entries(&self) -> &[EpochInfoV1Entry] {
        self.epoch_info.entries()
    }

    /// Committees for epochs `[0, snapshot_epoch + 1]`: the genesis committee
    /// plus one handed forward by each entry's `end_of_epoch_data`.
    pub fn committees(&self) -> &[Committee] {
        &self.committees
    }

    /// Convert the verified entries `[0, snapshot_epoch]` into `EpochInfoV2`
    /// index rows. Each row's `system_state` is the digest-verified start state
    /// of its own epoch (`start_system_states[i]`).
    pub(crate) fn into_epoch_info_v2_rows(self) -> Vec<EpochInfoV2> {
        let VerifiedEpochInfo {
            epoch_info,
            start_system_states,
            ..
        } = self;
        // `start_system_states[i]` is epoch `i`'s start state; `zip` drops the
        // trailing one (the state the last boundary proves, which has no row).
        // Each epoch's start checkpoint is the previous epoch's last + 1 (0 for
        // epoch 0), derived inline from the signed summaries.
        let mut previous_end_checkpoint: Option<u64> = None;
        epoch_info
            .into_entries()
            .into_iter()
            .zip(start_system_states)
            .map(|(entry, start_system_state)| {
                let start_checkpoint = previous_end_checkpoint.map_or(0, |seq| seq + 1);
                previous_end_checkpoint =
                    Some(*entry.last_checkpoint_summary.data().sequence_number());
                epoch_info_v2_row(entry, start_system_state, start_checkpoint)
            })
            .collect()
    }

    /// Restore the verified rows `[0, snapshot_epoch]` into the given
    /// consumer's epoch store.
    pub async fn restore_epoch_info(self, db: &impl RestoreEpochInfo) -> anyhow::Result<()> {
        let rows = self.into_epoch_info_v2_rows();
        db.restore_epoch_info(rows).await
    }
}

/// Verify a snapshot's `EPOCH_INFO` against the operator's trust roots: the
/// expected `chain_id`, the committee chain walked from `genesis_committee`,
/// and `genesis_system_state` (epoch 0's start state, which no entry proves).
/// Each entry must be the contiguous certified close of its epoch, signed by
/// the committee the previous entry handed forward, with its proof bundle
/// hashing back to the signed summary (see `verify_epoch_boundary_proof`).
/// Nothing is written; the returned `VerifiedEpochInfo` is the witness
/// consumers require.
pub fn verify_epoch_info_chain(
    epoch_info: EpochInfo,
    genesis_committee: Committee,
    genesis_system_state: IotaSystemState,
    snapshot_chain_id: ChainIdentifier,
    expected_chain_id: ChainIdentifier,
) -> anyhow::Result<VerifiedEpochInfo> {
    anyhow::ensure!(
        snapshot_chain_id == expected_chain_id,
        "snapshot chain_id {snapshot_chain_id} does not match this node's chain \
         {expected_chain_id} (snapshot from the wrong network's bucket?)"
    );
    anyhow::ensure!(
        genesis_committee.epoch == 0,
        "the trust root must be the genesis committee, got epoch {}",
        genesis_committee.epoch
    );

    let mut chain_verifier = CommitteeChainVerifier::new(genesis_committee);
    let mut committees = vec![chain_verifier.committee().clone()];
    // `start_system_states[i]` is epoch `i`'s start state; epoch 0's is the
    // genesis root, every later one is derived from the previous boundary.
    let mut start_system_states = vec![genesis_system_state];
    for (index, entry) in epoch_info.entries().iter().enumerate() {
        // EPOCH_INFO-specific: entries must be the contiguous epochs from 0.
        // The epoch comes from the signed summary, not a stored field, so it
        // can't disagree with the data it anchors.
        let epoch = entry.last_checkpoint_summary.epoch();
        anyhow::ensure!(
            epoch == index as u64,
            "EPOCH_INFO entry at index {index} carries a summary for epoch {epoch}",
        );

        // Defense in depth: the committee in epoch `index`'s start state must
        // match the one the chain certified — catches a tampered validator set
        // that left the (separately signed) committee handover intact.
        let start_committee = start_system_states[index].get_current_epoch_committee();
        anyhow::ensure!(
            start_committee.committee() == chain_verifier.committee(),
            "EPOCH_INFO entry for epoch {index}: the committee in its start system \
             state does not match the certified committee",
        );

        chain_verifier
            .verify_epoch_close(entry.last_checkpoint_summary.clone())
            .map_err(|e| {
                anyhow::anyhow!("EPOCH_INFO entry for epoch {index} failed verification: {e}")
            })?;

        // Anchor the rest of the entry to the now-verified summary and derive
        // epoch `index + 1`'s start state from the boundary objects.
        let next_start_state = verify_epoch_boundary_proof(entry)
            .map_err(|e| anyhow::anyhow!("EPOCH_INFO entry for epoch {index}: {e}"))?;

        committees.push(chain_verifier.committee().clone());
        start_system_states.push(next_start_state);
    }

    Ok(VerifiedEpochInfo {
        epoch_info,
        committees,
        start_system_states,
    })
}

/// Anchor an entry's proof bundle to its (already signature-verified)
/// `last_checkpoint_summary` and return the next epoch's digest-verified start
/// state — the system-state objects this boundary wrote. Each link (contents,
/// effects, events, start-state objects) is checked below.
fn verify_epoch_boundary_proof(entry: &EpochInfoV1Entry) -> anyhow::Result<IotaSystemState> {
    let summary = entry.last_checkpoint_summary.data();

    // 1. Contents hash to the signed summary.
    anyhow::ensure!(
        *entry.last_checkpoint_contents.digest() == summary.content_digest,
        "last_checkpoint_contents does not hash to the signed content_digest",
    );

    // 2. The epoch-change effects are the last tx of the verified contents.
    let expected_execution_digest = entry
        .last_checkpoint_contents
        .inner()
        .last()
        .ok_or_else(|| anyhow::anyhow!("the closing checkpoint has no transactions"))?;
    let effects = &entry.end_of_epoch_tx_effects;
    anyhow::ensure!(
        effects.execution_digests() == *expected_execution_digest,
        "end_of_epoch_tx_effects digest pair does not match the closing checkpoint's last transaction",
    );

    // 3. Events hash to the effects' events_digest (`None` ⇒ events empty, the
    // safe-mode boundary case).
    match effects.events_digest() {
        Some(events_digest) => anyhow::ensure!(
            entry.end_of_epoch_tx_events.digest() == *events_digest,
            "end_of_epoch_tx_events does not hash to the effects' events_digest",
        ),
        None => anyhow::ensure!(
            entry.end_of_epoch_tx_events.is_empty(),
            "the epoch-change effects carry no events_digest but \
             end_of_epoch_tx_events is non-empty",
        ),
    }

    // 4. Each start-state object's digest is one the effects wrote; `0x5` must
    // be present. Decode `IotaSystemState` only from these verified bytes.
    let written: HashSet<ObjectRef> = effects
        .all_changed_objects()
        .into_iter()
        .map(|(object_ref, _, _)| object_ref)
        .collect();
    let mut objects = Vec::with_capacity(entry.next_epoch_start_system_state_objects.len());
    for raw in &entry.next_epoch_start_system_state_objects {
        let object: Object = bcs::from_bytes(raw)
            .map_err(|e| anyhow::anyhow!("decoding a next-epoch start-state object: {e}"))?;
        anyhow::ensure!(
            written.contains(&object.object_ref()),
            "a next-epoch start-state object is not written by the epoch-change tx",
        );
        objects.push(object);
    }
    anyhow::ensure!(
        objects
            .iter()
            .any(|o| o.id() == IOTA_SYSTEM_STATE_OBJECT_ID),
        "the next-epoch start-state objects do not include the system-state object 0x5",
    );
    get_iota_system_state(&objects.as_slice())
        .map_err(|e| anyhow::anyhow!("decoding the next-epoch system state: {e}"))
}

/// Build an `EpochInfoV2` index row from a verified entry, its digest-verified
/// start system state, and its start checkpoint. The `epoch` comes from the
/// entry's signed summary; the `end_*` facts are derived from the embedded
/// entry by `EpochInfoV2`'s methods.
fn epoch_info_v2_row(
    entry: EpochInfoV1Entry,
    system_state: IotaSystemState,
    start_checkpoint: CheckpointSequenceNumber,
) -> EpochInfoV2 {
    EpochInfoV2 {
        epoch: entry.last_checkpoint_summary.epoch(),
        start_checkpoint,
        start_timestamp_ms: system_state.epoch_start_timestamp_ms(),
        system_state,
        epoch_close_proof: Some(entry),
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
