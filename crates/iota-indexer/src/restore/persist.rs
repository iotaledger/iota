// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use bytes::Bytes;
use fastcrypto::hash::{HashFunction, Sha3_256};
use iota_core::authority::authority_store_tables::LiveObject as SnapshotObject;
use iota_snapshot::{FileMetadata, VerifiedEpochInfo, reader::LiveObjectIter, restore::Restore};
use iota_storage::SHA3_BYTES;
use iota_types::iota_system_state::IotaSystemStateTrait;
use itertools::Itertools;

use crate::{
    chunk,
    errors::IndexerError,
    ingestion::{common::prepare::LiveObject, primary::persist::EpochToCommit},
    models::epoch::{EndOfEpochUpdate, StartOfEpochUpdate, extract_epoch_info_event},
    store::PgIndexerStore,
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
                "Sha does not match! expected: {expected_checksum:?}, actual: {sha3_digest:?}",
            );
            anyhow::bail!(
                "checksum verification failed for bucket/partition: {}/{}",
                file_metadata.bucket_num,
                file_metadata.part_num
            );
        }

        let persist_tasks = chunks
            .into_iter()
            .map(|c| self.spawn_blocking_task(move |this| this.persist_changed_objects(c)));
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
    /// as live ingestion does at an end-of-epoch checkpoint; the leading commit
    /// opens the genesis epoch.
    pub(crate) fn from_verified_epoch_info(verified_epoch_info: VerifiedEpochInfo) -> Vec<Self> {
        let (epoch_info, _committees, start_system_states) = verified_epoch_info.into_parts();
        let epoch_info_entries = epoch_info.into_entries();

        // Genesis epoch: open epoch 0 from its start state; no prior epoch to
        // close.
        let genesis_system_state =
            start_system_states[0].clone().into_iota_system_state_summary();
        let mut epochs_to_commit = vec![EpochToCommit {
            last_epoch: None,
            new_epoch: StartOfEpochUpdate::new(&genesis_system_state, 0, 0, None),
        }];

        for (epoch, epoch_info_entry) in epoch_info_entries.iter().enumerate() {
            let event = extract_epoch_info_event(&epoch_info_entry.end_of_epoch_tx_events)
                .unwrap_or_default();
            let last_checkpoint_summary = &epoch_info_entry.last_checkpoint_summary;
            let new_epoch_system_state =
                start_system_states[epoch + 1].clone().into_iota_system_state_summary();
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
