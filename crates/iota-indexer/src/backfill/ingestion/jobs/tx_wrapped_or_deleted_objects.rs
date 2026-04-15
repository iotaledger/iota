// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use diesel::RunQueryDsl;
use iota_types::full_checkpoint_content::CheckpointData;

use crate::{
    backfill::ingestion::IngestionBackfill,
    db::{ConnectionPool, get_pool_connection},
    errors::IndexerError,
    models::tx_indices::StoredTxWrappedOrDeletedObject,
    schema::tx_wrapped_or_deleted_objects,
};

pub(crate) struct TxWrappedOrDeletedObjectsBackfill;

#[async_trait::async_trait]
impl IngestionBackfill for TxWrappedOrDeletedObjectsBackfill {
    type ProcessedType = StoredTxWrappedOrDeletedObject;

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

        for (tx, (expected_digest, tx_sequence_number)) in transactions.iter().zip(tx_seq_numbers) {
            let actual_digest = tx.transaction.digest();

            if expected_digest != *actual_digest {
                return Err(IndexerError::FullNodeReading(format!(
                    "digest mismatch at checkpoint {checkpoint_seq}: expected {expected_digest}, found {actual_digest}",
                )));
            }

            results.extend(
                tx.effects
                    .all_tombstones()
                    .into_iter()
                    .chain(tx.effects.created_then_wrapped_objects())
                    .map(|(object_id, _)| StoredTxWrappedOrDeletedObject {
                        tx_sequence_number: tx_sequence_number as i64,
                        object_id: object_id.to_vec(),
                        sender: tx.transaction.sender_address().to_vec(),
                    }),
            );
        }

        Ok(results)
    }

    async fn persist_chunk(
        pool: ConnectionPool,
        processed_data: Vec<Self::ProcessedType>,
    ) -> Result<(), IndexerError> {
        let mut conn = get_pool_connection(&pool)?;

        diesel::insert_into(tx_wrapped_or_deleted_objects::table)
            .values(processed_data)
            .on_conflict_do_nothing()
            .execute(&mut conn)?;

        Ok(())
    }
}
