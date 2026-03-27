// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
use std::sync::Arc;

use diesel::{ExpressionMethods, RunQueryDsl};
use downcast::Any;
use iota_types::{effects::TransactionEffectsAPI, full_checkpoint_content::CheckpointData};

use crate::{
    Duration, IndexerMetrics, Registry, backfill::ingestion::IngestionBackfill, db::ConnectionPool,
    errors::IndexerError, ingestion::primary::prepare::PrimaryWorker,
    models::transactions::StoredTransaction, schema::transactions,
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
        let checkpoint_summary = &checkpoint.checkpoint_summary;
        let checkpoint_contents = &checkpoint.checkpoint_contents;
        let transactions = &checkpoint.transactions;
        let checkpoint_seq = checkpoint_summary.sequence_number;

        if checkpoint_contents.size() != transactions.len() {
            return Err(IndexerError::FullNodeReading(format!(
                "checkpoint content size mismatch at checkpoint {checkpoint_seq}: expected {}, found {}",
                checkpoint_contents.size(),
                transactions.len()
            )));
        }

        let tx_seq_numbers = checkpoint_contents
            .enumerate_transactions(checkpoint_summary)
            .map(|(seq, digest)| (digest.transaction, seq));

        let mut results = Vec::new();
        let dummy_metrics = IndexerMetrics::new(&Registry::new());

        // Only transactions with unwrapped objects need to be backfilled
        for (tx, (expected_digest, tx_sequence_number)) in transactions
            .iter()
            .zip(tx_seq_numbers)
            .filter(|(tx, _)| !tx.effects.unwrapped().is_empty())
        {
            let actual_digest = tx.transaction.digest();

            if expected_digest != *actual_digest {
                return Err(IndexerError::FullNodeReading(format!(
                    "digest mismatch at checkpoint {checkpoint_seq}: expected {expected_digest}, found {actual_digest}",
                )));
            }

            let indexed_tx = PrimaryWorker::index_transaction(
                tx,
                tx_sequence_number,
                checkpoint_seq,
                checkpoint_summary.timestamp_ms,
                &dummy_metrics,
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
        transactional_blocking_with_retry!(
            &pool,
            |conn| {
                for (tx_seq, obj_changes) in tx_sequence_numbers.iter().zip(object_changes.iter()) {
                    diesel::update(transactions::table)
                        .filter(transactions::tx_sequence_number.eq(tx_seq))
                        .set(transactions::object_changes.eq(obj_changes))
                        .execute(conn)?;
                }

                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
    }
}
