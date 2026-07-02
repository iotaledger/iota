// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use bytes::Bytes;
use fastcrypto::hash::{HashFunction, Sha3_256};
use iota_core::authority::authority_store_tables::LiveObject as SnapshotObject;
use iota_snapshot::{FileMetadata, VerifiedEpochInfo, reader::LiveObjectIter, restore::Restore};
use iota_storage::SHA3_BYTES;
use iota_types::{digests::ChainIdentifier, iota_system_state::IotaSystemStateTrait};
use itertools::Itertools;
use strum::IntoEnumIterator;

use crate::{
    chunk,
    errors::{IndexerError, IndexerResult},
    ingestion::{common::prepare::LiveObject, primary::persist::EpochToCommit},
    models::{
        checkpoints::StoredChainIdentifier,
        epoch::{EndOfEpochUpdate, StartOfEpochUpdate, extract_epoch_info_event},
    },
    pruning::pruner::PrunableTable,
    store::{IndexerStore, PgIndexerStore},
    types::IndexedCheckpoint,
};

impl Restore for PgIndexerStore {
    async fn insert_partition(
        &self,
        file_metadata: FileMetadata,
        bytes: Bytes,
        expected_checksum: &[u8; SHA3_BYTES],
    ) -> anyhow::Result<()> {
        let mut hasher = Sha3_256::default();
        let partition = LiveObjectIter::new(&file_metadata, bytes)?.scan(
            &mut hasher,
            |hasher,
             SnapshotObject {
                 object,
                 previous_transaction_checkpoint,
             }| {
                hasher.update(object.object_ref().digest.inner());
                let checkpoint_sequence_number =
                    previous_transaction_checkpoint.unwrap_or_default();
                Some(LiveObject::new(checkpoint_sequence_number, object))
            },
        );
        let chunks = chunk!(partition, self.config.parallel_objects_chunk_size);
        let sha3_digest = hasher.finalize().digest;
        if *expected_checksum != sha3_digest {
            tracing::error!(
                "sha does not match! expected: {expected_checksum:?}, actual: {sha3_digest:?}",
            );
            anyhow::bail!(
                "checksum verification failed for bucket/partition: {}/{}",
                file_metadata.bucket_num,
                file_metadata.part_num
            );
        }

        let persist_tasks = chunks
            .into_iter()
            .map(|c| self.spawn_blocking_task(move |this| this.persist_live_objects(c)));
        futures::future::try_join_all(persist_tasks)
            .await
            .map_err(|e| {
                tracing::error!(
                    "failed to join futures for persisting formal snapshot partition: {e}"
                );
                IndexerError::from(e)
            })?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                IndexerError::PostgresWrite(format!(
                    "failed to persist all formal snapshot object chunks: {e:?}",
                ))
            })?;
        Ok(())
    }
}

impl EpochToCommit {
    /// Builds the epoch commits for a verified snapshot.
    ///
    /// Returns one commit per epoch from 0 through `snapshot_epoch + 1`, in
    /// epoch order. Each entry closes its own epoch and opens the next, exactly
    /// as live ingestion does at an end-of-epoch checkpoint. The leading commit
    /// opens the genesis epoch.
    pub(crate) fn batch_from_verified_epoch_info(
        verified_epoch_info: VerifiedEpochInfo,
    ) -> Vec<Self> {
        let (epoch_info, _committees, start_system_states) = verified_epoch_info.into_parts();
        let epoch_info_entries = epoch_info.into_entries();

        // Initialize with the genesis epoch to commit.
        let genesis_system_state = start_system_states[0]
            .clone()
            .into_iota_system_state_summary();
        let mut epochs_to_commit = vec![EpochToCommit {
            last_epoch: None,
            new_epoch: StartOfEpochUpdate::new(&genesis_system_state, 0, 0, None),
        }];

        for (epoch, epoch_info_entry) in epoch_info_entries.iter().enumerate() {
            let event = extract_epoch_info_event(&epoch_info_entry.end_of_epoch_tx_events)
                .unwrap_or_default();
            let last_checkpoint_summary = &epoch_info_entry.last_checkpoint_summary;
            let new_epoch_system_state = start_system_states[epoch + 1]
                .clone()
                .into_iota_system_state_summary();
            let new_epoch_first_checkpoint_id = *last_checkpoint_summary.sequence_number() + 1;
            let new_epoch_first_tx_sequence_number =
                last_checkpoint_summary.network_total_transactions;
            epochs_to_commit.push(EpochToCommit {
                last_epoch: Some(EndOfEpochUpdate::new(last_checkpoint_summary, &event)),
                new_epoch: StartOfEpochUpdate::new(
                    &new_epoch_system_state,
                    new_epoch_first_checkpoint_id,
                    new_epoch_first_tx_sequence_number,
                    Some(&event),
                ),
            });
        }
        epochs_to_commit
    }
}

async fn populate_chain_id(store: &PgIndexerStore, chain_id: ChainIdentifier) -> IndexerResult<()> {
    let checkpoint_digest = chain_id.digest().into_inner().to_vec();
    store
        .execute_in_blocking_worker(|this| {
            this.persist_chain_identifier(StoredChainIdentifier { checkpoint_digest })
        })
        .await
}

async fn populate_protocol_and_feature_flags(
    store: &PgIndexerStore,
    chain_id: ChainIdentifier,
) -> IndexerResult<()> {
    let checkpoint_digest = chain_id.digest().into_inner().to_vec();
    store
        .execute_in_blocking_worker(move |this| {
            this.persist_protocol_configs_and_feature_flags(checkpoint_digest)
        })
        .await
}

async fn populate_epochs(
    store: &PgIndexerStore,
    verified_epoch_info: VerifiedEpochInfo,
) -> IndexerResult<()> {
    let epochs_to_commit = EpochToCommit::batch_from_verified_epoch_info(verified_epoch_info);
    store
        .execute_in_blocking_worker(move |this| this.persist_epochs(epochs_to_commit))
        .await
}

/// We populate the remaining tables after the objects.
///
/// This includes `epochs`, `chain_identifier` which we populate in parallel
/// with setting the checkpoint watermark to the last checkpoint of the snapshot
/// epoch.
///
/// Finally we populate `protocol_configs`, `feature_flags` up to the
/// protocol version that the snapshot epoch corresponds to, and the
/// `watermarks` table with the lower bounds associated with the epoch following
/// the snapshot.
pub(crate) async fn populate_remaining_tables(
    store: &PgIndexerStore,
    verified_epoch_info: VerifiedEpochInfo,
    snapshot_chain_id: ChainIdentifier,
) -> IndexerResult<()> {
    let snapshot_epoch_boundary = &verified_epoch_info
        .entries()
        .last()
        .expect("there should be an entry for the snapshot epoch");
    let sync_watermark = IndexedCheckpoint::from_iota_checkpoint(
        &snapshot_epoch_boundary.last_checkpoint_summary,
        &snapshot_epoch_boundary.last_checkpoint_contents,
        Default::default(), // We don't store this as part of the checkpoint so it's ok to set to 0
    );
    let pruning_watermarks: Vec<_> = PrunableTable::iter()
        .map(|table| (table, sync_watermark.epoch + 1))
        .collect();
    tokio::try_join!(
        populate_epochs(store, verified_epoch_info),
        populate_chain_id(store, snapshot_chain_id),
        store.persist_checkpoints(vec![sync_watermark]),
    )?;
    tokio::try_join!(
        populate_protocol_and_feature_flags(store, snapshot_chain_id),
        store.update_watermarks_lower_bound(pruning_watermarks.clone())
    )?;
    // finally align the lowest unpruned key with the lower bounds
    // to let the pruner know the pruning range start after restoring
    let (stored_watermarks, _) = store.get_watermarks().await?;
    let lowest_unpruned_keys = stored_watermarks
        .iter()
        .map(|watermark| {
            let table = PrunableTable::from(watermark);
            (table, table.pruning_strategy().range_end(watermark))
        })
        .collect::<Vec<_>>();
    store
        .update_watermarks_lowest_unpruned_key(lowest_unpruned_keys)
        .await
}
