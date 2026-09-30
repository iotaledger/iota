// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{sync::Arc, time::Duration};

use diesel::{ExpressionMethods, RunQueryDsl};
use downcast::Any;
use iota_types::{effects::TransactionEffectsAPI, full_checkpoint_content::CheckpointData};

use crate::{
    IndexerMetrics, Registry,
    backfill::ingestion::IngestionBackfill,
    db::ConnectionPool,
    errors::IndexerError,
    ingestion::{
        common::prepare::enumerate_checked_transactions, primary::prepare::index_transaction,
    },
    models::transactions::StoredTransaction,
    schema::transactions,
    store::diesel_macro::spawn_blocking_task,
    transactional_blocking_with_retry,
};

const PG_DB_COMMIT_SLEEP_DURATION: Duration = Duration::from_secs(3600);

pub(crate) struct ObjectChangesUnwrappedBackfill;

#[async_trait::async_trait]
impl IngestionBackfill for ObjectChangesUnwrappedBackfill {
    type ProcessedType = StoredTransaction;

    async fn process_checkpoint(
        checkpoint: Arc<CheckpointData>,
    ) -> Result<Vec<Self::ProcessedType>, IndexerError> {
        let mut results = Vec::new();
        let metrics = IndexerMetrics::new(&Registry::new());

        for check_result in enumerate_checked_transactions(&checkpoint)? {
            let (sequence_number, checkpoint_transaction) = check_result?;
            if checkpoint_transaction.effects.unwrapped().is_empty() {
                continue;
            }
            let indexed_tx = index_transaction(
                checkpoint_transaction,
                sequence_number,
                checkpoint.checkpoint_summary.sequence_number(),
                checkpoint.checkpoint_summary.timestamp_ms(),
                metrics.clone(),
            )
            .await?;
            results.push(StoredTransaction::from(&indexed_tx));
        }

        Ok(results)
    }

    async fn persist_chunk(
        pool: ConnectionPool,
        processed_data: Vec<Self::ProcessedType>,
    ) -> Result<(), IndexerError> {
        if processed_data.is_empty() {
            return Ok(());
        }

        let (tx_sequence_numbers, object_changes): (Vec<i64>, Vec<Vec<Option<Vec<u8>>>>) =
            processed_data
                .into_iter()
                .map(|tx| (tx.tx_sequence_number, tx.object_changes))
                .unzip();

        // The UPDATE only affects rows that exist in the database. Update for
        // non-existing rows is silently skipped.
        spawn_blocking_task(move || {
            transactional_blocking_with_retry!(
                &pool,
                |conn| {
                    for (tx_seq, obj_changes) in
                        tx_sequence_numbers.iter().zip(object_changes.iter())
                    {
                        diesel::update(transactions::table)
                            .filter(transactions::tx_sequence_number.eq(tx_seq))
                            .set(transactions::object_changes.eq(obj_changes))
                            .execute(conn)?;
                    }

                    Ok::<(), IndexerError>(())
                },
                PG_DB_COMMIT_SLEEP_DURATION
            )
        })
        .await
        .map_err(IndexerError::from)?
    }
}
