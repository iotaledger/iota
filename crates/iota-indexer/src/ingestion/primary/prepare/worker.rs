// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
use std::sync::Arc;

use async_trait::async_trait;
use iota_data_ingestion_core::Worker;
use iota_sdk_types::ObjectId;
use iota_types::{
    full_checkpoint_content::CheckpointData,
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
        common::{orchestration::OperationalLevel, prepare::ValidatedCheckpoint},
        primary::persist::{CheckpointDataToCommit, EpochToCommit, data},
    },
    metrics::IndexerMetrics,
    models::epoch::{EndOfEpochUpdate, StartOfEpochUpdate, extract_epoch_info_event},
    types::{IndexedCheckpoint, IndexerResult},
};

pub struct PrimaryWorker {
    metrics: IndexerMetrics,
    indexed_checkpoint_sender: iota_metrics::metered_channel::Sender<CheckpointDataToCommit>,
    operational_level: OperationalLevel,
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
            .send(transformer.transform(self.operational_level).await?)
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
        operational_level: OperationalLevel,
    ) -> Self {
        Self {
            metrics,
            indexed_checkpoint_sender,
            operational_level,
        }
    }
}

/// The builder of the data to commit to the database.
struct Transformer<'chk, 'm> {
    checkpoint: ValidatedCheckpoint<'chk>,
    metrics: &'m IndexerMetrics,
}

impl<'chk, 'm> Transformer<'chk, 'm> {
    fn new(checkpoint: ValidatedCheckpoint<'chk>, metrics: &'m IndexerMetrics) -> Self {
        Self {
            checkpoint,
            metrics,
        }
    }

    async fn transform(
        self,
        operational_level: OperationalLevel,
    ) -> IndexerResult<CheckpointDataToCommit> {
        info!(
            checkpoint_seq = self.checkpoint.sequence_number(),
            "Indexing checkpoint data blob"
        );

        let transaction_data = TransactionTransformer::new(self.checkpoint)
            .transform(self.metrics, operational_level)
            .await?;

        let event_data = EventsTransformer::new(self.checkpoint).transform(operational_level);

        let object_data =
            ObjectsTransformer::new(self.checkpoint).transform(self.metrics, operational_level);

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
            "Indexer lag: indexed checkpoint {time_now_ms} with time now {} and checkpoint time {}",
            checkpoint.sequence_number, checkpoint.timestamp_ms
        );
        let basic = data::Basic {
            checkpoint,
            transactions: transaction_data.transactions,
            displays: event_data.displays,
            object_changes: object_data.checkpoint_objects,
            object_versions: object_data.object_versions,
            packages: object_data.packages,
            epoch,
        };
        let objects_history = object_data
            .history_objects
            .map(|objects| data::ObjectsHistory {
                history_objects: objects,
            });
        let filtered_queries = transaction_data
            .transaction_indices
            .zip(event_data.events)
            .map(|(tx_indices, events)| data::FilteredQueries { tx_indices, events });
        if filtered_queries.is_none()
            && operational_level.includes(OperationalLevel::FilteredQueries)
        {
            return Err(IndexerError::DataTransformation(
                "missing tx indices or events for the filtered queries level".to_string(),
            ));
        }
        let combined_event_filters = event_data
            .event_indices
            .map(|event_indices| data::CombinedEventFilters { event_indices });

        Ok(CheckpointDataToCommit {
            basic,
            objects_history,
            filtered_queries,
            combined_event_filters,
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
