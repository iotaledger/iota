// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{sync::Arc, time::Duration};

use downcast::Any;
use iota_types::full_checkpoint_content::CheckpointData;

use crate::{
    backfill::ingestion::IngestionBackfill,
    db::ConnectionPool,
    errors::{Context, IndexerError},
    ingestion::{common::prepare::ValidatedCheckpoint, primary::prepare::ObjectsTransformer},
    insert_or_ignore_into,
    models::objects::StoredBackwardHistoryObject,
    schema::objects_backward_history,
    store::diesel_macro::spawn_blocking_task,
    transactional_blocking_with_retry,
};

const PG_DB_COMMIT_SLEEP_DURATION: Duration = Duration::from_secs(3600);

pub(crate) struct ObjectsBackwardHistoryBackfill;

#[async_trait::async_trait]
impl IngestionBackfill for ObjectsBackwardHistoryBackfill {
    type ProcessedType = StoredBackwardHistoryObject;

    async fn process_checkpoint(
        checkpoint: Arc<CheckpointData>,
    ) -> Result<Vec<Self::ProcessedType>, IndexerError> {
        let checkpoint = ValidatedCheckpoint::new(&checkpoint)?;
        Ok(ObjectsTransformer::new(checkpoint).build_history_objects())
    }

    async fn persist_chunk(
        pool: ConnectionPool,
        processed_data: Vec<Self::ProcessedType>,
    ) -> Result<(), IndexerError> {
        spawn_blocking_task(move || {
            transactional_blocking_with_retry!(
                &pool,
                |conn| {
                    insert_or_ignore_into!(objects_backward_history::table, &processed_data, conn);
                    Ok::<(), IndexerError>(())
                },
                PG_DB_COMMIT_SLEEP_DURATION
            )
        })
        .await
        .map_err(IndexerError::from)?
    }
}
