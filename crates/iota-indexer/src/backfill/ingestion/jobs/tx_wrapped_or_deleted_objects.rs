// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
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

    fn process_checkpoint(checkpoint: Arc<CheckpointData>) -> Vec<Self::ProcessedType> {
        let first_tx = checkpoint.checkpoint_summary.network_total_transactions as usize
            - checkpoint.transactions.len();

        checkpoint
            .transactions
            .iter()
            .enumerate()
            .flat_map(|(i, tx)| {
                let effects = &tx.effects;

                effects
                    .all_tombstones()
                    .into_iter()
                    .chain(effects.created_then_wrapped_objects())
                    .map(move |(object_id, _)| StoredTxWrappedOrDeletedObject {
                        tx_sequence_number: (first_tx + i) as i64,
                        object_id: object_id.to_vec(),
                        sender: tx.transaction.sender_address().to_vec(),
                    })
            })
            .collect()
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
