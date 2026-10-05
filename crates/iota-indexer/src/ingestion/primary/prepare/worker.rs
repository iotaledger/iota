// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use iota_data_ingestion_core::Worker;
use iota_sdk_types::ObjectId;
use iota_types::{
    full_checkpoint_content::{CheckpointData, CheckpointTransaction},
    iota_system_state::{IotaSystemStateTrait, get_iota_system_state},
    object::Object,
};
use tracing::{info, warn};

use super::{
    events::EventsTransformer, objects::ObjectsTransformer, transactions::TransactionTransformer,
};
use crate::{
    errors::IndexerError,
    ingestion::{
        common::prepare::ValidatedCheckpoint,
        primary::persist::{CheckpointDataToCommit, EpochToCommit},
    },
    metrics::IndexerMetrics,
    models::{
        display::StoredDisplay,
        epoch::{EndOfEpochUpdate, StartOfEpochUpdate, extract_epoch_info_event},
    },
    types::{EventIndex, IndexedCheckpoint, IndexedEvent, IndexerResult},
};

pub struct PrimaryWorker {
    metrics: IndexerMetrics,
    indexed_checkpoint_sender: iota_metrics::metered_channel::Sender<CheckpointDataToCommit>,
}

#[async_trait]
impl Worker for PrimaryWorker {
    type Message = ();
    type Error = IndexerError;

    async fn process_checkpoint(
        &self,
        checkpoint: Arc<CheckpointData>,
    ) -> Result<Self::Message, Self::Error> {
        self.metrics
            .latest_fullnode_checkpoint_sequence_number
            .set(checkpoint.checkpoint_summary.sequence_number as i64);
        let time_now_ms = chrono::Utc::now().timestamp_millis();
        let cp_download_lag = time_now_ms - checkpoint.checkpoint_summary.timestamp_ms as i64;
        info!(
            "checkpoint download lag for cp {}: {cp_download_lag} ms",
            checkpoint.checkpoint_summary.sequence_number
        );
        self.metrics.download_lag_ms.set(cp_download_lag);
        self.metrics
            .max_downloaded_checkpoint_sequence_number
            .set(checkpoint.checkpoint_summary.sequence_number as i64);
        self.metrics
            .downloaded_checkpoint_timestamp_ms
            .set(checkpoint.checkpoint_summary.timestamp_ms as i64);
        info!(
            "Indexer lag: downloaded checkpoint {} with time now {time_now_ms} and checkpoint time {}",
            checkpoint.checkpoint_summary.sequence_number,
            checkpoint.checkpoint_summary.timestamp_ms
        );

        let validated_checkpoint = ValidatedCheckpoint::new(&checkpoint)?;
        let transformer = Transformer::new(validated_checkpoint, &self.metrics);
        self.indexed_checkpoint_sender
            .send(transformer.transform().await?)
            .await
            .map_err(|_| {
                IndexerError::MpscChannel(
                    "failed to send checkpoint data, receiver half closed".into(),
                )
            })?;
        Ok(())
    }
}

impl PrimaryWorker {
    pub(crate) fn new(
        metrics: IndexerMetrics,
        indexed_checkpoint_sender: iota_metrics::metered_channel::Sender<CheckpointDataToCommit>,
    ) -> Self {
        Self {
            metrics,
            indexed_checkpoint_sender,
        }
    }
}

/// The builder of the data to commit to the database.
struct Transformer<'chk, 'm> {
    checkpoint: ValidatedCheckpoint<'chk>,
    metrics: &'m IndexerMetrics,
    events: Vec<IndexedEvent>,
    event_indices: Vec<EventIndex>,
    displays: BTreeMap<String, StoredDisplay>,
}

impl<'chk, 'm> Transformer<'chk, 'm> {
    fn new(checkpoint: ValidatedCheckpoint<'chk>, metrics: &'m IndexerMetrics) -> Self {
        Self {
            checkpoint,
            metrics,
            events: Default::default(),
            event_indices: Default::default(),
            displays: Default::default(),
        }
    }

    async fn transform(mut self) -> IndexerResult<CheckpointDataToCommit> {
        info!(
            checkpoint_seq = self.checkpoint.sequence_number(),
            "Indexing checkpoint data blob"
        );

        let transaction_data = TransactionTransformer::new(self.checkpoint)
            .transform(self.metrics)
            .await?;

        for (sequence_number, checkpoint_transaction) in self.checkpoint.enumerate_transactions() {
            self.extend_event_data(checkpoint_transaction, sequence_number);
        }

        let object_data = ObjectsTransformer::new(self.checkpoint).transform(self.metrics);

        let epoch = self.build_epoch()?;

        let total_successful_transactions: u64 = transaction_data
            .transactions
            .iter()
            .map(|tx| tx.successful_tx_num)
            .sum();
        let checkpoint = self.build_checkpoint(total_successful_transactions as usize);

        let time_now_ms = chrono::Utc::now().timestamp_millis();
        self.metrics
            .index_lag_ms
            .set(time_now_ms - checkpoint.timestamp_ms as i64);
        self.metrics
            .max_indexed_checkpoint_sequence_number
            .set(checkpoint.sequence_number as i64);
        self.metrics
            .indexed_checkpoint_timestamp_ms
            .set(checkpoint.timestamp_ms as i64);
        info!(
            "Indexer lag: indexed checkpoint {} with time now {} and checkpoint time {}",
            checkpoint.sequence_number, time_now_ms, checkpoint.timestamp_ms
        );

        Ok(CheckpointDataToCommit {
            checkpoint,
            transactions: transaction_data.transactions,
            events: self.events,
            event_indices: self.event_indices,
            tx_indices: transaction_data.transaction_indices,
            displays: self.displays,
            object_changes: object_data.checkpoint_objects,
            backward_history_changes: object_data.history_objects,
            object_versions: object_data.object_versions,
            packages: object_data.packages,
            epoch,
        })
    }

    fn build_checkpoint(&self, total_successful_transactions: usize) -> IndexedCheckpoint {
        let CheckpointData {
            checkpoint_summary,
            checkpoint_contents,
            ..
        } = self.checkpoint.data();

        IndexedCheckpoint::from_iota_checkpoint(
            checkpoint_summary,
            checkpoint_contents,
            total_successful_transactions,
        )
    }

    fn build_epoch(&self) -> IndexerResult<Option<EpochToCommit>> {
        let checkpoint_object_store = EpochEndIndexingObjectStore::new(self.checkpoint.data());

        let CheckpointData {
            transactions,
            checkpoint_summary,
            checkpoint_contents: _,
        } = self.checkpoint.data();

        // Genesis epoch
        if checkpoint_summary.sequence_number() == 0 {
            info!("Processing genesis epoch");
            let system_state =
                get_iota_system_state(&checkpoint_object_store)?.into_iota_system_state_summary();
            return Ok(Some(EpochToCommit {
                last_epoch: None,
                new_epoch: StartOfEpochUpdate::new(
                    &system_state,
                    0, // first_checkpoint_id
                    0, // first_tx_sequence_number
                    None,
                ),
            }));
        }

        // If not end of epoch, return
        if checkpoint_summary.end_of_epoch_data.is_none() {
            return Ok(None);
        }

        let event = transactions
            .iter()
            .find_map(|t| t.events.as_ref().and_then(extract_epoch_info_event));

        let system_state = get_iota_system_state(&checkpoint_object_store)?;
        if event.is_none() {
            warn!(
                "no SystemEpochInfoEvent found at end of epoch {}, some epoch data will be set to default.",
                checkpoint_summary.epoch,
            );
            assert!(
                system_state.safe_mode(),
                "iota is not in safe mode but no SystemEpochInfoEvent found at end of epoch {}",
                checkpoint_summary.epoch
            );
        }

        let event = event.unwrap_or_default();
        let new_epoch_first_checkpoint_id = checkpoint_summary.sequence_number + 1;
        let new_epoch_first_tx_sequence_number = checkpoint_summary.network_total_transactions;
        Ok(Some(EpochToCommit {
            last_epoch: Some(EndOfEpochUpdate::new(checkpoint_summary, &event)),
            new_epoch: StartOfEpochUpdate::new(
                &system_state.into_iota_system_state_summary(),
                new_epoch_first_checkpoint_id,
                new_epoch_first_tx_sequence_number,
                Some(&event),
            ),
        }))
    }

    fn extend_event_data(&mut self, transaction: &CheckpointTransaction, sequence_number: u64) {
        let transformer = EventsTransformer::new(
            transaction,
            sequence_number,
            self.checkpoint.sequence_number(),
            self.checkpoint.timestamp_ms(),
        );
        let event_data = transformer.transform();
        self.displays.extend(event_data.displays);
        self.events.extend(event_data.events);
        self.event_indices.extend(event_data.event_indices);
    }
}

/// Represents objects for end-of-epoch indexing.
/// Used to extract IotaSystemState and its dynamic children for end-of-epoch
/// indexing.
pub(crate) struct EpochEndIndexingObjectStore<'a> {
    objects: Vec<&'a Object>,
}

impl<'a> EpochEndIndexingObjectStore<'a> {
    pub fn new(data: &'a CheckpointData) -> Self {
        Self {
            objects: data.latest_live_output_objects(),
        }
    }
}

impl iota_types::storage::ObjectStore for EpochEndIndexingObjectStore<'_> {
    fn try_get_object(
        &self,
        object_id: &ObjectId,
    ) -> Result<Option<Object>, iota_types::storage::error::Error> {
        Ok(self
            .objects
            .iter()
            .find(|o| o.id() == *object_id)
            .cloned()
            .cloned())
    }

    fn try_get_object_by_key(
        &self,
        object_id: &ObjectId,
        version: iota_types::base_types::VersionNumber,
    ) -> Result<Option<Object>, iota_types::storage::error::Error> {
        Ok(self
            .objects
            .iter()
            .find(|o| o.id() == *object_id && o.version() == version)
            .cloned()
            .cloned())
    }
}
