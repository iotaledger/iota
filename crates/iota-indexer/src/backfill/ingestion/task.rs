// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{ops::RangeInclusive, sync::Arc};

use dashmap::DashMap;
use iota_data_ingestion_core::{ReaderOptions, setup_single_workflow};
use iota_types::messages_checkpoint::CheckpointSequenceNumber;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::{
    backfill::{
        Backfill,
        ingestion::{IngestionBackfill, adapter::Adapter},
    },
    db::ConnectionPool,
    errors::IndexerError,
};

// The amount of rows to update in one DB transaction
const PG_COMMIT_CHUNK_SIZE: usize = 100;

/// Orchestrates ingestion-driven backfill by buffering processed checkpoints
/// and coordinating range-based commits.
///
/// `IngestionBackfillTask` spins up an ingestion workflow that continuously
/// transforms checkpoints (via `Adapter<T>`), storing
/// them in `ready_checkpoints`. Backfill operations can then drain these
/// buffered records in order, pausing the backfill until the required
/// checkpoint data arrives (via `notify`), and commit the chunks.
pub struct IngestionBackfillTask<T: IngestionBackfill> {
    ready_checkpoints: Arc<DashMap<CheckpointSequenceNumber, Vec<T::ProcessedType>>>,
    notify: Arc<Notify>,
    _cancel_token: CancellationToken,
}

impl<T: IngestionBackfill + 'static> IngestionBackfillTask<T> {
    // Creates and starts a new ingestion‐driven backfill task using processor `T`.
    pub(crate) async fn new(
        remote_store_url: String,
        start_checkpoint: CheckpointSequenceNumber,
    ) -> Result<Self, IndexerError> {
        let ready_checkpoints = Arc::new(DashMap::new());
        let notify = Arc::new(Notify::new());
        let adapter: Adapter<T> = Adapter {
            ready_checkpoints: ready_checkpoints.clone(),
            notify: notify.clone(),
        };
        let reader_options = ReaderOptions {
            batch_size: 200,
            ..Default::default()
        };
        let (executor, _cancel_token) = setup_single_workflow(
            adapter,
            remote_store_url,
            start_checkpoint,
            200,
            Some(reader_options),
        )
        .await?;

        tokio::spawn(async move {
            if let Err(join_err) = executor.await {
                error!(?join_err, "Ingestion executor panicked or was cancelled");
            }
        });

        Ok(Self {
            ready_checkpoints,
            notify,
            _cancel_token,
        })
    }
}

#[async_trait::async_trait]
impl<T: IngestionBackfill> Backfill for IngestionBackfillTask<T> {
    async fn backfill_range(
        &self,
        pool: ConnectionPool,
        range: &RangeInclusive<usize>,
    ) -> Result<(), IndexerError> {
        let mut processed_data = vec![];
        let mut start = *range.start();
        let end = *range.end();

        while start <= end {
            if let Some((_, processed)) = self
                .ready_checkpoints
                .remove(&(start as CheckpointSequenceNumber))
            {
                processed_data.extend(processed);
                start += 1;
            } else {
                info!("Waiting for processed data for checkpoint sequence number {start}");
                self.notify.notified().await;
            }
        }

        info!(
            "Persisting backfill chunk from {} to {} with {} total items",
            range.start(),
            range.end(),
            processed_data.len()
        );

        // Limit the size of each chunk.
        // postgres has a parameter limit of 65535, meaning that row_count * col_count
        // <= 65535.
        while !processed_data.is_empty() {
            let batch: Vec<_> = processed_data
                .drain(..processed_data.len().min(PG_COMMIT_CHUNK_SIZE))
                .collect();

            T::persist_chunk(pool.clone(), batch).await?;
        }

        Ok(())
    }
}

#[cfg(feature = "pg_integration")]
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use diesel::{RunQueryDsl, sql_query, sql_types::BigInt};
    use iota_types::{
        full_checkpoint_content::CheckpointData, messages_checkpoint::CheckpointSequenceNumber,
    };
    use tokio::sync::Notify;
    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::{
        db::get_pool_connection,
        test_utils::{RowCount, TestDatabase, db_url},
    };

    struct BackfillDummyTable;
    #[async_trait::async_trait]
    impl IngestionBackfill for BackfillDummyTable {
        type ProcessedType = usize;

        fn process_checkpoint(
            checkpoint: Arc<CheckpointData>,
        ) -> Result<Vec<Self::ProcessedType>, IndexerError> {
            Ok(vec![checkpoint.checkpoint_summary.sequence_number as usize])
        }

        async fn persist_chunk(
            pool: ConnectionPool,
            processed_data: Vec<Self::ProcessedType>,
        ) -> Result<(), IndexerError> {
            let mut conn = get_pool_connection(&pool)?;

            for id in processed_data {
                sql_query("INSERT INTO ingestion_items (id) VALUES ($1) ON CONFLICT DO NOTHING")
                    .bind::<BigInt, _>(id as i64)
                    .execute(&mut conn)?;
            }

            Ok(())
        }
    }

    fn setup_target(pool: &ConnectionPool) {
        let mut conn = get_pool_connection(pool).unwrap();

        // Create ingestion_items table
        sql_query(
            r#"
            CREATE TABLE IF NOT EXISTS ingestion_items (
                id BIGINT PRIMARY KEY
            )
            "#,
        )
        .execute(&mut conn)
        .unwrap();
    }

    #[tokio::test]
    async fn ingestion_backfill_writes_to_db() {
        telemetry_subscribers::init_for_testing();

        let mut db = TestDatabase::new(db_url("ingestion_backfill_test"));
        db.recreate();
        db.reset_db();

        {
            let pool = db.to_connection_pool();
            setup_target(&pool);

            // Create an IngestionBackfillTask without remote workflow
            let ready_checkpoints = Arc::new(DashMap::new());
            let notify = Arc::new(Notify::new());
            let cancel = CancellationToken::new();
            let task = IngestionBackfillTask::<BackfillDummyTable> {
                ready_checkpoints: ready_checkpoints.clone(),
                notify: notify.clone(),
                _cancel_token: cancel,
            };

            // Simulate ready checkpoints for backfill
            for seq in 0..20 {
                ready_checkpoints.insert(seq as CheckpointSequenceNumber, vec![seq]);
            }

            // Perform backfill for checkpoint 0..=4
            task.backfill_range(pool.clone(), &(0..=4))
                .await
                .expect("Backfill failed for checkpoint range 0..=4");

            // Validate checkpoints 0..=4 are consumed
            for seq in 0..=4 {
                assert!(
                    !ready_checkpoints.contains_key(&seq),
                    "Checkpoint {} should have been consumed",
                    seq
                );
            }
            // Validate checkpoints 5..=19 are still present
            for seq in 5..=19 {
                assert!(
                    ready_checkpoints.contains_key(&seq),
                    "Checkpoint {} should still be present",
                    seq
                );
            }

            assert_eq!(15, ready_checkpoints.len());

            // Consume the rest of the checkpoints
            task.backfill_range(pool.clone(), &(5..=19))
                .await
                .expect("Backfill failed for checkpoint range 5..=19");

            assert!(
                ready_checkpoints.is_empty(),
                "All checkpoints should have been consumed"
            );

            // Check if the data was written correctly
            let mut conn = pool.get().unwrap();
            let RowCount { cnt } = sql_query("SELECT COUNT(*) AS cnt FROM ingestion_items")
                .get_result(&mut conn)
                .unwrap();
            assert_eq!(cnt, 20, "Should have 20 items in ingestion_items table");
        }

        db.drop_if_exists();
    }
}
