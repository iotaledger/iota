// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use core::result::Result::Ok;
use std::{
    any::Any as StdAny,
    collections::{BTreeMap, HashMap},
    time::Duration,
};

use async_trait::async_trait;
use diesel::{
    ExpressionMethods, OptionalExtension, PgConnection, QueryDsl, RunQueryDsl,
    dsl::{max, min, sql},
    sql_types::{Array, BigInt, Bytea, Nullable, SmallInt, Text},
    upsert::excluded,
};
use downcast::Any;
use iota_protocol_config::ProtocolConfig;
use iota_types::{
    base_types::ObjectID,
    digests::{ChainIdentifier, CheckpointDigest},
    messages_checkpoint::CheckpointSequenceNumber,
};
use itertools::Itertools;
use strum::IntoEnumIterator;
use tap::TapFallible;
use tracing::info;

use super::pg_partition_manager::{EpochPartitionData, PgPartitionManager};
use crate::{
    blocking_call_is_ok_or_panic,
    db::ConnectionPool,
    errors::{Context, IndexerError, IndexerResult},
    ingestion::{
        common::{
            persist::{CommitterWatermark, ObjectsSnapshotHandlerTables},
            prepare::{
                CheckpointObjectChanges, LiveObject, RemovedObject,
                retain_latest_objects_from_checkpoint_batch,
            },
        },
        primary::persist::{EpochToCommit, TransactionObjectChangesToCommit},
    },
    insert_or_ignore_into,
    metrics::IndexerMetrics,
    models::{
        checkpoints::{StoredChainIdentifier, StoredCheckpoint, StoredCpTx},
        display::StoredDisplay,
        epoch::{StoredEpochInfo, StoredFeatureFlag, StoredProtocolConfig},
        events::StoredEvent,
        obj_indices::StoredObjectVersion,
        objects::{
            StoredDeletedObject, StoredHistoryObject, StoredObject, StoredObjectSnapshot,
            StoredObjects,
        },
        packages::StoredPackage,
        transactions::{
            CheckpointTxGlobalOrder, IndexStatus, OptimisticTransaction, StoredTransaction,
        },
        tx_indices::TxIndexSplit,
        watermarks::StoredWatermark,
    },
    on_conflict_do_update, on_conflict_do_update_with_condition, persist_chunk_into_table,
    persist_chunk_into_table_in_existing_connection,
    pruning::pruner::PrunableTable,
    read_only_blocking, run_query, run_query_with_retry,
    schema::{
        chain_identifier, checkpoints, display, epochs, event_emit_module, event_emit_package,
        event_senders, event_struct_instantiation, event_struct_module, event_struct_name,
        event_struct_package, events, feature_flags, objects, objects_history, objects_snapshot,
        objects_version, optimistic_transactions, packages, protocol_configs, pruner_cp_watermark,
        transactions, tx_calls_fun, tx_calls_mod, tx_calls_pkg, tx_changed_objects, tx_digests,
        tx_global_order, tx_input_objects, tx_kinds, tx_recipients, tx_senders,
        tx_wrapped_or_deleted_objects, watermarks,
    },
    store::{IndexerStore, diesel_macro::mark_in_blocking_pool},
    transactional_blocking_with_retry,
    types::{
        EventIndex, IndexedCheckpoint, IndexedDeletedObject, IndexedEvent, IndexedObject,
        IndexedPackage, IndexedTransaction, TxIndex,
    },
};

/// A cursor representing the global order position of transaction according to
/// tx_global_order table
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TxGlobalOrderCursor {
    pub global_sequence_number: i64,
    pub optimistic_sequence_number: i64,
}

#[macro_export]
macro_rules! chunk {
    ($data: expr, $size: expr) => {{
        $data
            .into_iter()
            .chunks($size)
            .into_iter()
            .map(|c| c.collect())
            .collect::<Vec<Vec<_>>>()
    }};
}

macro_rules! prune_tx_or_event_indice_table {
    ($table:ident, $conn:expr, $min_tx:expr, $max_tx:expr, $context_msg:expr) => {
        diesel::delete($table::table.filter($table::tx_sequence_number.between($min_tx, $max_tx)))
            .execute($conn)
            .map_err(IndexerError::from)
            .context($context_msg)?;
    };
}

// In one DB transaction, the update could be chunked into
// a few statements, this is the amount of rows to update in one statement
// TODO: I think with the `per_db_tx` params, `PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX`
// is now less relevant. We should do experiments and remove it if it's true.
const PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX: usize = 1000;
// The amount of rows to update in one DB transaction
const PG_COMMIT_PARALLEL_CHUNK_SIZE: usize = 100;
// The amount of rows to update in one DB transaction, for objects particularly
// Having this number too high may cause many db deadlocks because of
// optimistic locking.
const PG_COMMIT_OBJECTS_PARALLEL_CHUNK_SIZE: usize = 500;
const PG_DB_COMMIT_SLEEP_DURATION: Duration = Duration::from_secs(3600);

#[derive(Clone)]
pub struct PgIndexerStoreConfig {
    pub parallel_chunk_size: usize,
    pub parallel_objects_chunk_size: usize,
}

pub struct PgIndexerStore {
    blocking_cp: ConnectionPool,
    metrics: IndexerMetrics,
    partition_manager: PgPartitionManager,
    config: PgIndexerStoreConfig,
}

impl Clone for PgIndexerStore {
    fn clone(&self) -> PgIndexerStore {
        Self {
            blocking_cp: self.blocking_cp.clone(),
            metrics: self.metrics.clone(),
            partition_manager: self.partition_manager.clone(),
            config: self.config.clone(),
        }
    }
}

impl PgIndexerStore {
    pub fn new(blocking_cp: ConnectionPool, metrics: IndexerMetrics) -> Self {
        let parallel_chunk_size = std::env::var("PG_COMMIT_PARALLEL_CHUNK_SIZE")
            .unwrap_or_else(|_e| PG_COMMIT_PARALLEL_CHUNK_SIZE.to_string())
            .parse::<usize>()
            .unwrap();
        let parallel_objects_chunk_size = std::env::var("PG_COMMIT_OBJECTS_PARALLEL_CHUNK_SIZE")
            .unwrap_or_else(|_e| PG_COMMIT_OBJECTS_PARALLEL_CHUNK_SIZE.to_string())
            .parse::<usize>()
            .unwrap();
        let partition_manager = PgPartitionManager::new(blocking_cp.clone())
            .expect("failed to initialize partition manager");
        let config = PgIndexerStoreConfig {
            parallel_chunk_size,
            parallel_objects_chunk_size,
        };

        Self {
            blocking_cp,
            metrics,
            partition_manager,
            config,
        }
    }

    pub fn get_metrics(&self) -> IndexerMetrics {
        self.metrics.clone()
    }

    pub fn blocking_cp(&self) -> ConnectionPool {
        self.blocking_cp.clone()
    }

    pub(crate) async fn get_latest_epoch_id_in_blocking_worker(
        &self,
    ) -> Result<Option<u64>, IndexerError> {
        self.execute_in_blocking_worker(move |this| this.get_latest_epoch_id())
            .await
    }

    pub fn get_latest_epoch_id(&self) -> Result<Option<u64>, IndexerError> {
        read_only_blocking!(&self.blocking_cp, |conn| {
            epochs::dsl::epochs
                .select(max(epochs::epoch))
                .first::<Option<i64>>(conn)
                .map(|v| v.map(|v| v as u64))
        })
        .context("Failed reading latest epoch id from PostgresDB")
    }

    /// Get the range of the protocol versions that need to be indexed.
    pub fn get_protocol_version_index_range(&self) -> Result<(i64, i64), IndexerError> {
        // We start indexing from the next protocol version after the latest one stored
        // in the db.
        let start = read_only_blocking!(&self.blocking_cp, |conn| {
            protocol_configs::dsl::protocol_configs
                .select(max(protocol_configs::protocol_version))
                .first::<Option<i64>>(conn)
        })
        .context("Failed reading latest protocol version from PostgresDB")?
        .map_or(1, |v| v + 1);

        // We end indexing at the protocol version of the latest epoch stored in the db.
        let end = read_only_blocking!(&self.blocking_cp, |conn| {
            epochs::dsl::epochs
                .select(max(epochs::protocol_version))
                .first::<Option<i64>>(conn)
        })
        .context("Failed reading latest epoch protocol version from PostgresDB")?
        .unwrap_or(1);
        Ok((start, end))
    }

    pub fn get_chain_identifier(&self) -> Result<Option<Vec<u8>>, IndexerError> {
        read_only_blocking!(&self.blocking_cp, |conn| {
            chain_identifier::dsl::chain_identifier
                .select(chain_identifier::checkpoint_digest)
                .first::<Vec<u8>>(conn)
                .optional()
        })
        .context("Failed reading chain id from PostgresDB")
    }

    fn get_latest_checkpoint_sequence_number(&self) -> Result<Option<u64>, IndexerError> {
        read_only_blocking!(&self.blocking_cp, |conn| {
            checkpoints::dsl::checkpoints
                .select(max(checkpoints::sequence_number))
                .first::<Option<i64>>(conn)
                .map(|v| v.map(|v| v as u64))
        })
        .context("Failed reading latest checkpoint sequence number from PostgresDB")
    }

    fn get_available_checkpoint_range(&self) -> Result<(u64, u64), IndexerError> {
        read_only_blocking!(&self.blocking_cp, |conn| {
            checkpoints::dsl::checkpoints
                .select((
                    min(checkpoints::sequence_number),
                    max(checkpoints::sequence_number),
                ))
                .first::<(Option<i64>, Option<i64>)>(conn)
                .map(|(min, max)| {
                    (
                        min.unwrap_or_default() as u64,
                        max.unwrap_or_default() as u64,
                    )
                })
        })
        .context("Failed reading min and max checkpoint sequence numbers from PostgresDB")
    }

    fn get_prunable_epoch_range(&self) -> Result<(u64, u64), IndexerError> {
        read_only_blocking!(&self.blocking_cp, |conn| {
            epochs::dsl::epochs
                .select((min(epochs::epoch), max(epochs::epoch)))
                .first::<(Option<i64>, Option<i64>)>(conn)
                .map(|(min, max)| {
                    (
                        min.unwrap_or_default() as u64,
                        max.unwrap_or_default() as u64,
                    )
                })
        })
        .context("Failed reading min and max epoch numbers from PostgresDB")
    }

    fn get_min_prunable_checkpoint(&self) -> Result<u64, IndexerError> {
        read_only_blocking!(&self.blocking_cp, |conn| {
            pruner_cp_watermark::dsl::pruner_cp_watermark
                .select(min(pruner_cp_watermark::checkpoint_sequence_number))
                .first::<Option<i64>>(conn)
                .map(|v| v.unwrap_or_default() as u64)
        })
        .context("Failed reading min prunable checkpoint sequence number from PostgresDB")
    }

    fn get_checkpoint_range_for_epoch(
        &self,
        epoch: u64,
    ) -> Result<(u64, Option<u64>), IndexerError> {
        read_only_blocking!(&self.blocking_cp, |conn| {
            epochs::dsl::epochs
                .select((epochs::first_checkpoint_id, epochs::last_checkpoint_id))
                .filter(epochs::epoch.eq(epoch as i64))
                .first::<(i64, Option<i64>)>(conn)
                .map(|(min, max)| (min as u64, max.map(|v| v as u64)))
        })
        .context(
            format!("failed reading checkpoint range from PostgresDB for epoch {epoch}").as_str(),
        )
    }

    fn get_transaction_range_for_checkpoint(
        &self,
        checkpoint: u64,
    ) -> Result<(u64, u64), IndexerError> {
        read_only_blocking!(&self.blocking_cp, |conn| {
            pruner_cp_watermark::dsl::pruner_cp_watermark
                .select((
                    pruner_cp_watermark::min_tx_sequence_number,
                    pruner_cp_watermark::max_tx_sequence_number,
                ))
                .filter(pruner_cp_watermark::checkpoint_sequence_number.eq(checkpoint as i64))
                .first::<(i64, i64)>(conn)
                .map(|(min, max)| (min as u64, max as u64))
        })
        .context(
            format!("failed reading transaction range from PostgresDB for checkpoint {checkpoint}")
                .as_str(),
        )
    }

    pub(crate) async fn get_global_order_for_tx_seq_in_blocking_worker(
        &self,
        tx_seq: i64,
    ) -> Result<TxGlobalOrderCursor, IndexerError> {
        self.execute_in_blocking_worker(move |this| this.get_global_order_for_tx_seq(tx_seq))
            .await
    }

    fn get_global_order_for_tx_seq(
        &self,
        tx_seq: i64,
    ) -> Result<TxGlobalOrderCursor, IndexerError> {
        let result = read_only_blocking!(&self.blocking_cp, |conn| {
            tx_global_order::dsl::tx_global_order
                .select((
                    tx_global_order::global_sequence_number,
                    tx_global_order::optimistic_sequence_number,
                ))
                .filter(tx_global_order::chk_tx_sequence_number.eq(tx_seq))
                .first::<(i64, i64)>(conn)
        })
        .context(
            format!("failed reading global sequence number from PostgresDB for tx seq {tx_seq}")
                .as_str(),
        )?;
        let (global_sequence_number, optimistic_sequence_number) = result;
        Ok(TxGlobalOrderCursor {
            global_sequence_number,
            optimistic_sequence_number,
        })
    }

    pub(crate) async fn prune_optimistic_transactions_up_to_in_blocking_worker(
        &self,
        to: TxGlobalOrderCursor,
        limit: i64,
    ) -> IndexerResult<usize> {
        self.execute_in_blocking_worker(move |this| {
            this.prune_optimistic_transactions_up_to(to, limit)
        })
        .await
    }

    fn prune_optimistic_transactions_up_to(
        &self,
        to: TxGlobalOrderCursor,
        limit: i64,
    ) -> IndexerResult<usize> {
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                let sql = r#"
                    WITH ids_to_delete AS (
                         SELECT global_sequence_number, optimistic_sequence_number
                         FROM optimistic_transactions
                         WHERE (global_sequence_number, optimistic_sequence_number) <= ($1, $2)
                         ORDER BY global_sequence_number, optimistic_sequence_number
                         FOR UPDATE LIMIT $3
                     )
                     DELETE FROM optimistic_transactions otx
                     USING ids_to_delete
                     WHERE (otx.global_sequence_number, otx.optimistic_sequence_number) =
                           (ids_to_delete.global_sequence_number, ids_to_delete.optimistic_sequence_number)
                "#;
                diesel::sql_query(sql)
                    .bind::<BigInt, _>(to.global_sequence_number)
                    .bind::<BigInt, _>(to.optimistic_sequence_number)
                    .bind::<BigInt, _>(limit)
                    .execute(conn)
                    .map_err(IndexerError::from)
                    .context(
                        format!("failed to prune optimistic_transactions table to {to:?} with limit {limit}").as_str(),
                    )
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
    }

    fn get_latest_object_snapshot_watermark(
        &self,
    ) -> Result<Option<CommitterWatermark>, IndexerError> {
        read_only_blocking!(&self.blocking_cp, |conn| {
            watermarks::table
                .select((
                    watermarks::epoch_hi_inclusive,
                    watermarks::checkpoint_hi_inclusive,
                    watermarks::tx_hi,
                ))
                .filter(
                    watermarks::entity
                        .eq(ObjectsSnapshotHandlerTables::ObjectsSnapshot.to_string()),
                )
                .first::<(i64, i64, i64)>(conn)
                // Handle case where the watermark is not set yet
                .optional()
                .map(|v| {
                    v.map(|(epoch, cp, tx)| CommitterWatermark {
                        epoch_hi_inclusive: epoch as u64,
                        checkpoint_hi_inclusive: cp as u64,
                        tx_hi: tx as u64,
                    })
                })
        })
        .context("Failed reading latest object snapshot watermark from PostgresDB")
    }

    fn get_latest_object_snapshot_checkpoint_sequence_number(
        &self,
    ) -> Result<Option<CheckpointSequenceNumber>, IndexerError> {
        read_only_blocking!(&self.blocking_cp, |conn| {
            objects_snapshot::table
                .select(max(objects_snapshot::checkpoint_sequence_number))
                .first::<Option<i64>>(conn)
                .map(|v| v.map(|v| v as CheckpointSequenceNumber))
        })
        .context("Failed reading latest object snapshot checkpoint sequence number from PostgresDB")
    }

    fn persist_display_updates(
        &self,
        display_updates: BTreeMap<String, StoredDisplay>,
    ) -> Result<(), IndexerError> {
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            {
                let value = display_updates.values().collect::<Vec<_>>();
                |conn| self.persist_displays_in_existing_transaction(conn, value)
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )?;

        Ok(())
    }

    fn persist_changed_objects(&self, objects: Vec<LiveObject>) -> Result<(), IndexerError> {
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_objects_chunks
            .start_timer();
        let len = objects.len();
        let raw_query = r#"
            INSERT INTO objects (
                object_id,
                object_version,
                object_digest,
                owner_type,
                owner_id,
                object_type,
                object_type_package,
                object_type_module,
                object_type_name,
                serialized_object,
                coin_type,
                coin_balance,
                df_kind
            )
            SELECT
                u.object_id,
                u.object_version,
                u.object_digest,
                u.owner_type,
                u.owner_id,
                u.object_type,
                u.object_type_package,
                u.object_type_module,
                u.object_type_name,
                u.serialized_object,
                u.coin_type,
                u.coin_balance,
                u.df_kind
            FROM UNNEST(
                $1::BYTEA[],
                $2::BIGINT[],
                $3::BYTEA[],
                $4::SMALLINT[],
                $5::BYTEA[],
                $6::TEXT[],
                $7::BYTEA[],
                $8::TEXT[],
                $9::TEXT[],
                $10::BYTEA[],
                $11::TEXT[],
                $12::BIGINT[],
                $13::SMALLINT[],
                $14::BYTEA[]
            ) AS u(object_id, object_version, object_digest, owner_type, owner_id, object_type, object_type_package, object_type_module, object_type_name, serialized_object, coin_type, coin_balance, df_kind, tx_digest)
            LEFT JOIN tx_global_order o ON o.tx_digest = u.tx_digest
            WHERE o.optimistic_sequence_number IS NULL OR o.optimistic_sequence_number = 0
            ON CONFLICT (object_id) DO UPDATE
            SET
                object_version = EXCLUDED.object_version,
                object_digest = EXCLUDED.object_digest,
                owner_type = EXCLUDED.owner_type,
                owner_id = EXCLUDED.owner_id,
                object_type = EXCLUDED.object_type,
                object_type_package = EXCLUDED.object_type_package,
                object_type_module = EXCLUDED.object_type_module,
                object_type_name = EXCLUDED.object_type_name,
                serialized_object = EXCLUDED.serialized_object,
                coin_type = EXCLUDED.coin_type,
                coin_balance = EXCLUDED.coin_balance,
                df_kind = EXCLUDED.df_kind
        "#;
        let (objects, tx_digests): (StoredObjects, Vec<_>) = objects
            .into_iter()
            .map(LiveObject::split)
            .map(|(indexed_object, tx_digest)| {
                (
                    StoredObject::from(indexed_object),
                    tx_digest.into_inner().to_vec(),
                )
            })
            .unzip();
        let query = diesel::sql_query(raw_query)
            .bind::<Array<Bytea>, _>(objects.object_ids)
            .bind::<Array<BigInt>, _>(objects.object_versions)
            .bind::<Array<Bytea>, _>(objects.object_digests)
            .bind::<Array<SmallInt>, _>(objects.owner_types)
            .bind::<Array<Nullable<Bytea>>, _>(objects.owner_ids)
            .bind::<Array<Nullable<Text>>, _>(objects.object_types)
            .bind::<Array<Nullable<Bytea>>, _>(objects.object_type_packages)
            .bind::<Array<Nullable<Text>>, _>(objects.object_type_modules)
            .bind::<Array<Nullable<Text>>, _>(objects.object_type_names)
            .bind::<Array<Bytea>, _>(objects.serialized_objects)
            .bind::<Array<Nullable<Text>>, _>(objects.coin_types)
            .bind::<Array<Nullable<BigInt>>, _>(objects.coin_balances)
            .bind::<Array<Nullable<SmallInt>>, _>(objects.df_kinds)
            .bind::<Array<Bytea>, _>(tx_digests);
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                query.clone().execute(conn)?;
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
        .tap_ok(|_| {
            let elapsed = guard.stop_and_record();
            info!(elapsed, "Persisted {len} chunked objects");
        })
        .tap_err(|e| {
            tracing::error!("failed to persist object mutations with error: {e}");
        })
    }

    fn persist_removed_objects(&self, objects: Vec<RemovedObject>) -> Result<(), IndexerError> {
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_objects_chunks
            .start_timer();
        let len = objects.len();
        let raw_query = r#"
            DELETE FROM objects
            WHERE object_id IN (
                SELECT u.object_id
                FROM UNNEST(
                    $1::BYTEA[],
                    $2::BYTEA[]
                ) AS u(object_id, tx_digest)
                LEFT JOIN tx_global_order o ON o.tx_digest = u.tx_digest
                WHERE o.optimistic_sequence_number IS NULL OR o.optimistic_sequence_number = 0
            )
        "#;
        let (object_ids, tx_digests): (Vec<_>, Vec<_>) = objects
            .into_iter()
            .map(|removed_object| {
                (
                    removed_object.object_id().to_vec(),
                    removed_object.transaction_digest.into_inner().to_vec(),
                )
            })
            .unzip();
        let query = diesel::sql_query(raw_query)
            .bind::<Array<Bytea>, _>(object_ids)
            .bind::<Array<Bytea>, _>(tx_digests);
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                query.clone().execute(conn)?;
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
        .tap_ok(|_| {
            let elapsed = guard.stop_and_record();
            info!(elapsed, "Deleted {len} chunked objects");
        })
        .tap_err(|e| {
            tracing::error!("failed to persist object deletions with error: {e}");
        })
    }

    fn persist_object_mutation_chunk_in_existing_transaction(
        &self,
        conn: &mut PgConnection,
        mutated_object_mutation_chunk: Vec<StoredObject>,
    ) -> Result<(), IndexerError> {
        on_conflict_do_update!(
            objects::table,
            mutated_object_mutation_chunk,
            objects::object_id,
            (
                objects::object_id.eq(excluded(objects::object_id)),
                objects::object_version.eq(excluded(objects::object_version)),
                objects::object_digest.eq(excluded(objects::object_digest)),
                objects::owner_type.eq(excluded(objects::owner_type)),
                objects::owner_id.eq(excluded(objects::owner_id)),
                objects::object_type.eq(excluded(objects::object_type)),
                objects::serialized_object.eq(excluded(objects::serialized_object)),
                objects::coin_type.eq(excluded(objects::coin_type)),
                objects::coin_balance.eq(excluded(objects::coin_balance)),
                objects::df_kind.eq(excluded(objects::df_kind)),
            ),
            conn
        );
        Ok::<(), IndexerError>(())
    }

    fn persist_object_deletion_chunk_in_existing_transaction(
        &self,
        conn: &mut PgConnection,
        deleted_objects_chunk: Vec<StoredDeletedObject>,
    ) -> Result<(), IndexerError> {
        diesel::delete(
            objects::table.filter(
                objects::object_id.eq_any(
                    deleted_objects_chunk
                        .iter()
                        .map(|o| o.object_id.clone())
                        .collect::<Vec<_>>(),
                ),
            ),
        )
        .execute(conn)
        .map_err(IndexerError::from)
        .context("Failed to write object deletion to PostgresDB")?;

        Ok::<(), IndexerError>(())
    }

    fn backfill_objects_snapshot_chunk(
        &self,
        objects_snapshot: Vec<StoredObjectSnapshot>,
    ) -> Result<(), IndexerError> {
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_objects_snapshot_chunks
            .start_timer();
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                for objects_snapshot_chunk in
                    objects_snapshot.chunks(PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX)
                {
                    on_conflict_do_update!(
                        objects_snapshot::table,
                        objects_snapshot_chunk,
                        objects_snapshot::object_id,
                        (
                            objects_snapshot::object_version
                                .eq(excluded(objects_snapshot::object_version)),
                            objects_snapshot::object_status
                                .eq(excluded(objects_snapshot::object_status)),
                            objects_snapshot::object_digest
                                .eq(excluded(objects_snapshot::object_digest)),
                            objects_snapshot::checkpoint_sequence_number
                                .eq(excluded(objects_snapshot::checkpoint_sequence_number)),
                            objects_snapshot::owner_type.eq(excluded(objects_snapshot::owner_type)),
                            objects_snapshot::owner_id.eq(excluded(objects_snapshot::owner_id)),
                            objects_snapshot::object_type_package
                                .eq(excluded(objects_snapshot::object_type_package)),
                            objects_snapshot::object_type_module
                                .eq(excluded(objects_snapshot::object_type_module)),
                            objects_snapshot::object_type_name
                                .eq(excluded(objects_snapshot::object_type_name)),
                            objects_snapshot::object_type
                                .eq(excluded(objects_snapshot::object_type)),
                            objects_snapshot::serialized_object
                                .eq(excluded(objects_snapshot::serialized_object)),
                            objects_snapshot::coin_type.eq(excluded(objects_snapshot::coin_type)),
                            objects_snapshot::coin_balance
                                .eq(excluded(objects_snapshot::coin_balance)),
                            objects_snapshot::df_kind.eq(excluded(objects_snapshot::df_kind)),
                        ),
                        conn
                    );
                }
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
        .tap_ok(|_| {
            let elapsed = guard.stop_and_record();
            info!(
                elapsed,
                "Persisted {} chunked objects snapshot",
                objects_snapshot.len(),
            );
        })
        .tap_err(|e| {
            tracing::error!("failed to persist object snapshot with error: {e}");
        })
    }

    fn persist_objects_history_chunk(
        &self,
        stored_objects_history: Vec<StoredHistoryObject>,
    ) -> Result<(), IndexerError> {
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_objects_history_chunks
            .start_timer();
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                for stored_objects_history_chunk in
                    stored_objects_history.chunks(PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX)
                {
                    insert_or_ignore_into!(
                        objects_history::table,
                        stored_objects_history_chunk,
                        conn
                    );
                }
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
        .tap_ok(|_| {
            let elapsed = guard.stop_and_record();
            info!(
                elapsed,
                "Persisted {} chunked objects history",
                stored_objects_history.len(),
            );
        })
        .tap_err(|e| {
            tracing::error!("failed to persist object history with error: {e}");
        })
    }

    fn persist_object_version_chunk(
        &self,
        object_versions: Vec<StoredObjectVersion>,
    ) -> Result<(), IndexerError> {
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_objects_version_chunks
            .start_timer();

        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                for object_version_chunk in object_versions.chunks(PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX)
                {
                    insert_or_ignore_into!(objects_version::table, object_version_chunk, conn);
                }
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
        .tap_ok(|_| {
            let elapsed = guard.stop_and_record();
            info!(
                elapsed,
                "Persisted {} chunked object versions",
                object_versions.len(),
            );
        })
        .tap_err(|e| {
            tracing::error!("failed to persist object versions with error: {e}");
        })
    }

    fn persist_checkpoints(&self, checkpoints: Vec<IndexedCheckpoint>) -> Result<(), IndexerError> {
        let Some(first_checkpoint) = checkpoints.first() else {
            return Ok(());
        };

        // If the first checkpoint has sequence number 0, we need to persist the digest
        // as chain identifier.
        if first_checkpoint.sequence_number == 0 {
            let checkpoint_digest = first_checkpoint.checkpoint_digest.into_inner().to_vec();
            self.persist_protocol_configs_and_feature_flags(checkpoint_digest)?;
            transactional_blocking_with_retry!(
                &self.blocking_cp,
                |conn| {
                    let checkpoint_digest =
                        first_checkpoint.checkpoint_digest.into_inner().to_vec();
                    insert_or_ignore_into!(
                        chain_identifier::table,
                        StoredChainIdentifier { checkpoint_digest },
                        conn
                    );
                    Ok::<(), IndexerError>(())
                },
                PG_DB_COMMIT_SLEEP_DURATION
            )?;
        }
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_checkpoints
            .start_timer();

        let stored_cp_txs = checkpoints.iter().map(StoredCpTx::from).collect::<Vec<_>>();
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                for stored_cp_tx_chunk in stored_cp_txs.chunks(PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX) {
                    insert_or_ignore_into!(pruner_cp_watermark::table, stored_cp_tx_chunk, conn);
                }
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
        .tap_ok(|_| {
            info!(
                "Persisted {} pruner_cp_watermark rows.",
                stored_cp_txs.len(),
            );
        })
        .tap_err(|e| {
            tracing::error!("failed to persist pruner_cp_watermark with error: {e}");
        })?;

        let stored_checkpoints = checkpoints
            .iter()
            .map(StoredCheckpoint::from)
            .collect::<Vec<_>>();
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                for stored_checkpoint_chunk in
                    stored_checkpoints.chunks(PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX)
                {
                    insert_or_ignore_into!(checkpoints::table, stored_checkpoint_chunk, conn);
                    let time_now_ms = chrono::Utc::now().timestamp_millis();
                    for stored_checkpoint in stored_checkpoint_chunk {
                        self.metrics
                            .db_commit_lag_ms
                            .set(time_now_ms - stored_checkpoint.timestamp_ms);
                        self.metrics.max_committed_checkpoint_sequence_number.set(
                            stored_checkpoint.sequence_number,
                        );
                        self.metrics.committed_checkpoint_timestamp_ms.set(
                            stored_checkpoint.timestamp_ms,
                        );
                    }
                    for stored_checkpoint in stored_checkpoint_chunk {
                        info!("Indexer lag: persisted checkpoint {} with time now {} and checkpoint time {}", stored_checkpoint.sequence_number, time_now_ms, stored_checkpoint.timestamp_ms);
                    }
                }
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
        .tap_ok(|_| {
            let elapsed = guard.stop_and_record();
            info!(
                elapsed,
                "Persisted {} checkpoints",
                stored_checkpoints.len()
            );
        })
        .tap_err(|e| {
            tracing::error!("failed to persist checkpoints with error: {e}");
        })
    }

    fn persist_transactions_chunk(
        &self,
        transactions: Vec<IndexedTransaction>,
    ) -> Result<(), IndexerError> {
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_transactions_chunks
            .start_timer();
        let transformation_guard = self
            .metrics
            .checkpoint_db_commit_latency_transactions_chunks_transformation
            .start_timer();
        let transactions = transactions
            .iter()
            .map(StoredTransaction::from)
            .collect::<Vec<_>>();
        drop(transformation_guard);

        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                for transaction_chunk in transactions.chunks(PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX) {
                    insert_or_ignore_into!(transactions::table, transaction_chunk, conn);
                }
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
        .tap_ok(|_| {
            let elapsed = guard.stop_and_record();
            info!(
                elapsed,
                "Persisted {} chunked transactions",
                transactions.len()
            );
        })
        .tap_err(|e| {
            tracing::error!("failed to persist transactions with error: {e}");
        })
    }

    fn persist_tx_global_order_chunk(
        &self,
        tx_order: Vec<CheckpointTxGlobalOrder>,
    ) -> Result<(), IndexerError> {
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_tx_insertion_order_chunks
            .start_timer();

        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                for tx_order_chunk in tx_order.chunks(PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX) {
                    insert_or_ignore_into!(tx_global_order::table, tx_order_chunk, conn);
                }
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
        .tap_ok(|_| {
            let elapsed = guard.stop_and_record();
            info!(
                elapsed,
                "Persisted {} chunked txs insertion order",
                tx_order.len()
            );
        })
        .tap_err(|e| {
            tracing::error!("failed to persist txs insertion order with error: {e}");
        })
    }

    /// We enforce index-status semantics for checkpointed transactions
    /// in `tx_global_order`.
    ///
    /// Namely, checkpointed transactions (i.e. with `optimistic_sequence_number
    /// == 0`) are updated to `optimistic_sequence_number == -1` to indicate
    /// that they have been persisted in the database.
    fn update_status_for_checkpoint_transactions_chunk(
        &self,
        tx_order: Vec<CheckpointTxGlobalOrder>,
    ) -> Result<(), IndexerError> {
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_tx_insertion_order_chunks
            .start_timer();

        let num_transactions = tx_order.len();
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                on_conflict_do_update_with_condition!(
                    tx_global_order::table,
                    tx_order.clone(),
                    tx_global_order::tx_digest,
                    tx_global_order::optimistic_sequence_number.eq(IndexStatus::Completed),
                    tx_global_order::optimistic_sequence_number.eq(IndexStatus::Started),
                    conn
                );
                on_conflict_do_update_with_condition!(
                    tx_global_order::table,
                    tx_order.clone(),
                    tx_global_order::tx_digest,
                    tx_global_order::chk_tx_sequence_number
                        .eq(excluded(tx_global_order::chk_tx_sequence_number)),
                    tx_global_order::chk_tx_sequence_number.is_null(),
                    conn
                );
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
        .tap_ok(|_| {
            let elapsed = guard.stop_and_record();
            info!(
                elapsed,
                "Updated {} chunked values of `tx_global_order`", num_transactions
            );
        })
        .tap_err(|e| {
            tracing::error!("failed to update `tx_global_order` with error: {e}");
        })
    }

    fn persist_events_chunk(&self, events: Vec<IndexedEvent>) -> Result<(), IndexerError> {
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_events_chunks
            .start_timer();
        let len = events.len();
        let events = events
            .into_iter()
            .map(StoredEvent::from)
            .collect::<Vec<_>>();

        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                for event_chunk in events.chunks(PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX) {
                    insert_or_ignore_into!(events::table, event_chunk, conn);
                }
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
        .tap_ok(|_| {
            let elapsed = guard.stop_and_record();
            info!(elapsed, "Persisted {} chunked events", len);
        })
        .tap_err(|e| {
            tracing::error!("failed to persist events with error: {e}");
        })
    }

    fn persist_packages(&self, packages: Vec<IndexedPackage>) -> Result<(), IndexerError> {
        if packages.is_empty() {
            return Ok(());
        }
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_packages
            .start_timer();
        let packages = packages
            .into_iter()
            .map(StoredPackage::from)
            .collect::<Vec<_>>();
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                for packages_chunk in packages.chunks(PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX) {
                    on_conflict_do_update!(
                        packages::table,
                        packages_chunk,
                        packages::package_id,
                        (
                            packages::package_id.eq(excluded(packages::package_id)),
                            packages::move_package.eq(excluded(packages::move_package)),
                        ),
                        conn
                    );
                }
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
        .tap_ok(|_| {
            let elapsed = guard.stop_and_record();
            info!(elapsed, "Persisted {} packages", packages.len());
        })
        .tap_err(|e| {
            tracing::error!("failed to persist packages with error: {e}");
        })
    }

    async fn persist_event_indices_chunk(
        &self,
        indices: Vec<EventIndex>,
    ) -> Result<(), IndexerError> {
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_event_indices_chunks
            .start_timer();
        let len = indices.len();
        let (
            event_emit_packages,
            event_emit_modules,
            event_senders,
            event_struct_packages,
            event_struct_modules,
            event_struct_names,
            event_struct_instantiations,
        ) = indices.into_iter().map(|i| i.split()).fold(
            (
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ),
            |(
                mut event_emit_packages,
                mut event_emit_modules,
                mut event_senders,
                mut event_struct_packages,
                mut event_struct_modules,
                mut event_struct_names,
                mut event_struct_instantiations,
            ),
             index| {
                event_emit_packages.push(index.0);
                event_emit_modules.push(index.1);
                event_senders.push(index.2);
                event_struct_packages.push(index.3);
                event_struct_modules.push(index.4);
                event_struct_names.push(index.5);
                event_struct_instantiations.push(index.6);
                (
                    event_emit_packages,
                    event_emit_modules,
                    event_senders,
                    event_struct_packages,
                    event_struct_modules,
                    event_struct_names,
                    event_struct_instantiations,
                )
            },
        );

        // Now persist all the event indices in parallel into their tables.
        let mut futures = vec![];
        futures.push(self.spawn_blocking_task(move |this| {
            persist_chunk_into_table!(
                event_emit_package::table,
                event_emit_packages,
                &this.blocking_cp
            )
        }));

        futures.push(self.spawn_blocking_task(move |this| {
            persist_chunk_into_table!(
                event_emit_module::table,
                event_emit_modules,
                &this.blocking_cp
            )
        }));

        futures.push(self.spawn_blocking_task(move |this| {
            persist_chunk_into_table!(event_senders::table, event_senders, &this.blocking_cp)
        }));

        futures.push(self.spawn_blocking_task(move |this| {
            persist_chunk_into_table!(
                event_struct_package::table,
                event_struct_packages,
                &this.blocking_cp
            )
        }));

        futures.push(self.spawn_blocking_task(move |this| {
            persist_chunk_into_table!(
                event_struct_module::table,
                event_struct_modules,
                &this.blocking_cp
            )
        }));

        futures.push(self.spawn_blocking_task(move |this| {
            persist_chunk_into_table!(
                event_struct_name::table,
                event_struct_names,
                &this.blocking_cp
            )
        }));

        futures.push(self.spawn_blocking_task(move |this| {
            persist_chunk_into_table!(
                event_struct_instantiation::table,
                event_struct_instantiations,
                &this.blocking_cp
            )
        }));

        futures::future::try_join_all(futures)
            .await
            .map_err(|e| {
                tracing::error!("failed to join event indices futures in a chunk: {e}");
                IndexerError::from(e)
            })?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                IndexerError::PostgresWrite(format!(
                    "Failed to persist all event indices in a chunk: {e:?}"
                ))
            })?;
        let elapsed = guard.stop_and_record();
        info!(elapsed, "Persisted {} chunked event indices", len);
        Ok(())
    }

    async fn persist_tx_indices_chunk_v2(&self, indices: Vec<TxIndex>) -> Result<(), IndexerError> {
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_tx_indices_chunks
            .start_timer();
        let len = indices.len();

        let splits: Vec<TxIndexSplit> = indices.into_iter().map(Into::into).collect();

        let senders: Vec<_> = splits.iter().flat_map(|ix| ix.tx_senders.clone()).collect();
        let recipients: Vec<_> = splits
            .iter()
            .flat_map(|ix| ix.tx_recipients.clone())
            .collect();
        let input_objects: Vec<_> = splits
            .iter()
            .flat_map(|ix| ix.tx_input_objects.clone())
            .collect();
        let changed_objects: Vec<_> = splits
            .iter()
            .flat_map(|ix| ix.tx_changed_objects.clone())
            .collect();
        let wrapped_or_deleted_objects: Vec<_> = splits
            .iter()
            .flat_map(|ix| ix.tx_wrapped_or_deleted_objects.clone())
            .collect();
        let pkgs: Vec<_> = splits.iter().flat_map(|ix| ix.tx_pkgs.clone()).collect();
        let mods: Vec<_> = splits.iter().flat_map(|ix| ix.tx_mods.clone()).collect();
        let funs: Vec<_> = splits.iter().flat_map(|ix| ix.tx_funs.clone()).collect();
        let digests: Vec<_> = splits.iter().flat_map(|ix| ix.tx_digests.clone()).collect();
        let kinds: Vec<_> = splits.iter().flat_map(|ix| ix.tx_kinds.clone()).collect();

        let futures = [
            self.spawn_blocking_task(move |this| {
                persist_chunk_into_table!(tx_senders::table, senders, &this.blocking_cp)
            }),
            self.spawn_blocking_task(move |this| {
                persist_chunk_into_table!(tx_recipients::table, recipients, &this.blocking_cp)
            }),
            self.spawn_blocking_task(move |this| {
                persist_chunk_into_table!(tx_input_objects::table, input_objects, &this.blocking_cp)
            }),
            self.spawn_blocking_task(move |this| {
                persist_chunk_into_table!(
                    tx_changed_objects::table,
                    changed_objects,
                    &this.blocking_cp
                )
            }),
            self.spawn_blocking_task(move |this| {
                persist_chunk_into_table!(
                    tx_wrapped_or_deleted_objects::table,
                    wrapped_or_deleted_objects,
                    &this.blocking_cp
                )
            }),
            self.spawn_blocking_task(move |this| {
                persist_chunk_into_table!(tx_calls_pkg::table, pkgs, &this.blocking_cp)
            }),
            self.spawn_blocking_task(move |this| {
                persist_chunk_into_table!(tx_calls_mod::table, mods, &this.blocking_cp)
            }),
            self.spawn_blocking_task(move |this| {
                persist_chunk_into_table!(tx_calls_fun::table, funs, &this.blocking_cp)
            }),
            self.spawn_blocking_task(move |this| {
                persist_chunk_into_table!(tx_digests::table, digests, &this.blocking_cp)
            }),
            self.spawn_blocking_task(move |this| {
                persist_chunk_into_table!(tx_kinds::table, kinds, &this.blocking_cp)
            }),
        ];

        futures::future::try_join_all(futures)
            .await
            .map_err(|e| {
                tracing::error!("failed to join tx indices futures in a chunk: {e}");
                IndexerError::from(e)
            })?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                IndexerError::PostgresWrite(format!(
                    "Failed to persist all tx indices in a chunk: {e:?}"
                ))
            })?;
        let elapsed = guard.stop_and_record();
        info!(elapsed, "Persisted {} chunked tx_indices", len);
        Ok(())
    }

    fn persist_epoch(&self, epoch: EpochToCommit) -> Result<(), IndexerError> {
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_epoch
            .start_timer();
        let epoch_id = epoch.new_epoch.epoch;

        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                if let Some(last_epoch) = &epoch.last_epoch {
                    info!(last_epoch.epoch, "Persisting epoch end data.");
                    diesel::update(epochs::table.filter(epochs::epoch.eq(last_epoch.epoch)))
                        .set(last_epoch)
                        .execute(conn)?;
                }

                info!(epoch.new_epoch.epoch, "Persisting epoch beginning info");
                insert_or_ignore_into!(epochs::table, &epoch.new_epoch, conn);
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
        .tap_ok(|_| {
            let elapsed = guard.stop_and_record();
            info!(elapsed, epoch_id, "Persisted epoch beginning info");
        })
        .tap_err(|e| {
            tracing::error!("failed to persist epoch with error: {e}");
        })
    }

    fn advance_epoch(&self, epoch_to_commit: EpochToCommit) -> Result<(), IndexerError> {
        let last_epoch_id = epoch_to_commit.last_epoch.as_ref().map(|e| e.epoch);
        // partition_0 has been created, so no need to advance it.
        if let Some(last_epoch_id) = last_epoch_id {
            let last_db_epoch: Option<StoredEpochInfo> =
                read_only_blocking!(&self.blocking_cp, |conn| {
                    epochs::table
                        .filter(epochs::epoch.eq(last_epoch_id))
                        .first::<StoredEpochInfo>(conn)
                        .optional()
                })
                .context("Failed to read last epoch from PostgresDB")?;
            if let Some(last_epoch) = last_db_epoch {
                let epoch_partition_data =
                    EpochPartitionData::compose_data(epoch_to_commit, last_epoch);
                let table_partitions = self.partition_manager.get_table_partitions()?;
                for (table, (_, last_partition)) in table_partitions {
                    // Only advance epoch partition for epoch partitioned tables.
                    if !self
                        .partition_manager
                        .get_strategy(&table)
                        .is_epoch_partitioned()
                    {
                        continue;
                    }
                    let guard = self.metrics.advance_epoch_latency.start_timer();
                    self.partition_manager.advance_epoch(
                        table.clone(),
                        last_partition,
                        &epoch_partition_data,
                    )?;
                    let elapsed = guard.stop_and_record();
                    info!(
                        elapsed,
                        "Advanced epoch partition {} for table {}",
                        last_partition,
                        table.clone()
                    );
                }
            } else {
                tracing::error!("last epoch: {last_epoch_id} from PostgresDB is None.");
            }
        }

        Ok(())
    }

    fn prune_checkpoints_table(&self, cp: u64) -> Result<(), IndexerError> {
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                diesel::delete(
                    checkpoints::table.filter(checkpoints::sequence_number.eq(cp as i64)),
                )
                .execute(conn)
                .map_err(IndexerError::from)
                .context("Failed to prune checkpoints table")?;

                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
    }

    fn prune_event_indices_table(&self, min_tx: u64, max_tx: u64) -> Result<(), IndexerError> {
        let (min_tx, max_tx) = (min_tx as i64, max_tx as i64);
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                prune_tx_or_event_indice_table!(
                    event_emit_module,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune event_emit_module table"
                );
                prune_tx_or_event_indice_table!(
                    event_emit_package,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune event_emit_package table"
                );
                prune_tx_or_event_indice_table![
                    event_senders,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune event_senders table"
                ];
                prune_tx_or_event_indice_table![
                    event_struct_instantiation,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune event_struct_instantiation table"
                ];
                prune_tx_or_event_indice_table![
                    event_struct_module,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune event_struct_module table"
                ];
                prune_tx_or_event_indice_table![
                    event_struct_name,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune event_struct_name table"
                ];
                prune_tx_or_event_indice_table![
                    event_struct_package,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune event_struct_package table"
                ];
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
    }

    fn prune_tx_indices_table(&self, min_tx: u64, max_tx: u64) -> Result<(), IndexerError> {
        let (min_tx, max_tx) = (min_tx as i64, max_tx as i64);
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                prune_tx_or_event_indice_table!(
                    tx_senders,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune tx_senders table"
                );
                prune_tx_or_event_indice_table!(
                    tx_recipients,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune tx_recipients table"
                );
                prune_tx_or_event_indice_table![
                    tx_input_objects,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune tx_input_objects table"
                ];
                prune_tx_or_event_indice_table![
                    tx_changed_objects,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune tx_changed_objects table"
                ];
                prune_tx_or_event_indice_table![
                    tx_wrapped_or_deleted_objects,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune tx_wrapped_or_deleted_objects table"
                ];
                prune_tx_or_event_indice_table![
                    tx_calls_pkg,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune tx_calls_pkg table"
                ];
                prune_tx_or_event_indice_table![
                    tx_calls_mod,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune tx_calls_mod table"
                ];
                prune_tx_or_event_indice_table![
                    tx_calls_fun,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune tx_calls_fun table"
                ];
                prune_tx_or_event_indice_table![
                    tx_digests,
                    conn,
                    min_tx,
                    max_tx,
                    "Failed to prune tx_digests table"
                ];
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
    }

    fn prune_cp_tx_table(&self, cp: u64) -> Result<(), IndexerError> {
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                diesel::delete(
                    pruner_cp_watermark::table
                        .filter(pruner_cp_watermark::checkpoint_sequence_number.eq(cp as i64)),
                )
                .execute(conn)
                .map_err(IndexerError::from)
                .context("Failed to prune pruner_cp_watermark table")?;
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
    }

    fn get_network_total_transactions_by_end_of_epoch(
        &self,
        epoch: u64,
    ) -> Result<Option<u64>, IndexerError> {
        read_only_blocking!(&self.blocking_cp, |conn| {
            epochs::table
                .filter(epochs::epoch.eq(epoch as i64))
                .select(epochs::network_total_transactions)
                .get_result::<Option<i64>>(conn)
        })
        .context(format!("failed to get network total transactions in epoch {epoch}").as_str())
        .map(|option| option.map(|v| v as u64))
    }

    fn refresh_participation_metrics(&self) -> Result<(), IndexerError> {
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                diesel::sql_query("REFRESH MATERIALIZED VIEW participation_metrics")
                    .execute(conn)?;
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
        .tap_ok(|_| {
            info!("Successfully refreshed participation_metrics");
        })
        .tap_err(|e| {
            tracing::error!("failed to refresh participation_metrics: {e}");
        })
    }

    fn update_watermarks_upper_bound<E: IntoEnumIterator>(
        &self,
        watermark: CommitterWatermark,
    ) -> Result<(), IndexerError>
    where
        E::Iterator: Iterator<Item: AsRef<str>>,
    {
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_watermarks
            .start_timer();

        let upper_bound_updates = E::iter()
            .map(|table| StoredWatermark::from_upper_bound_update(table.as_ref(), watermark))
            .collect::<Vec<_>>();

        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                diesel::insert_into(watermarks::table)
                    .values(&upper_bound_updates)
                    .on_conflict(watermarks::entity)
                    .do_update()
                    .set((
                        watermarks::epoch_hi_inclusive.eq(excluded(watermarks::epoch_hi_inclusive)),
                        watermarks::checkpoint_hi_inclusive
                            .eq(excluded(watermarks::checkpoint_hi_inclusive)),
                        watermarks::tx_hi.eq(excluded(watermarks::tx_hi)),
                    ))
                    .execute(conn)
                    .map_err(IndexerError::from)
                    .context("Failed to update watermarks upper bound")?;
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
        .tap_ok(|_| {
            let elapsed = guard.stop_and_record();
            info!(elapsed, "Persisted watermarks");
        })
        .tap_err(|e| {
            tracing::error!("Failed to persist watermarks with error: {}", e);
        })
    }

    fn map_epochs_to_cp_tx(
        &self,
        epochs: &[u64],
    ) -> Result<HashMap<u64, (u64, u64)>, IndexerError> {
        let pool = &self.blocking_cp;
        let results: Vec<(i64, i64, i64)> = run_query!(pool, move |conn| {
            epochs::table
                .filter(epochs::epoch.eq_any(epochs.iter().map(|&e| e as i64)))
                .select((
                    epochs::epoch,
                    epochs::first_checkpoint_id,
                    epochs::first_tx_sequence_number,
                ))
                .load::<(i64, i64, i64)>(conn)
        })
        .context("Failed to fetch first checkpoint and tx seq num for epochs")?;

        Ok(results
            .into_iter()
            .map(|(epoch, checkpoint, tx)| (epoch as u64, (checkpoint as u64, tx as u64)))
            .collect())
    }

    fn update_watermarks_lower_bound(
        &self,
        watermarks: Vec<(PrunableTable, u64)>,
    ) -> Result<(), IndexerError> {
        use diesel::query_dsl::methods::FilterDsl;

        let epochs: Vec<u64> = watermarks.iter().map(|(_table, epoch)| *epoch).collect();
        let epoch_mapping = self.map_epochs_to_cp_tx(&epochs)?;
        let lookups: Result<Vec<StoredWatermark>, IndexerError> = watermarks
            .into_iter()
            .map(|(table, epoch)| {
                let (checkpoint, tx) = epoch_mapping.get(&epoch).ok_or_else(|| {
                    IndexerError::PersistentStorageDataCorruption(format!(
                        "epoch {epoch} not found in epoch mapping",
                    ))
                })?;
                Ok(StoredWatermark::from_lower_bound_update(
                    table.as_ref(),
                    epoch,
                    table.select_reader_lo(*checkpoint, *tx),
                ))
            })
            .collect();
        let lower_bound_updates = lookups?;
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_watermarks
            .start_timer();
        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                diesel::insert_into(watermarks::table)
                    .values(&lower_bound_updates)
                    .on_conflict(watermarks::entity)
                    .do_update()
                    .set((
                        watermarks::reader_lo.eq(excluded(watermarks::reader_lo)),
                        watermarks::epoch_lo.eq(excluded(watermarks::epoch_lo)),
                        watermarks::timestamp_ms.eq(sql::<diesel::sql_types::BigInt>(
                            "(EXTRACT(EPOCH FROM CURRENT_TIMESTAMP) * 1000)::bigint",
                        )),
                    ))
                    .filter(excluded(watermarks::reader_lo).gt(watermarks::reader_lo))
                    .filter(excluded(watermarks::epoch_lo).gt(watermarks::epoch_lo))
                    .filter(
                        diesel::dsl::sql::<diesel::sql_types::BigInt>(
                            "(EXTRACT(EPOCH FROM CURRENT_TIMESTAMP) * 1000)::bigint",
                        )
                        .gt(watermarks::timestamp_ms),
                    )
                    .execute(conn)
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
        .tap_ok(|_| {
            let elapsed = guard.stop_and_record();
            info!(elapsed, "Persisted watermarks");
        })
        .tap_err(|e| {
            tracing::error!("Failed to persist watermarks with error: {}", e);
        })?;
        Ok(())
    }

    fn get_watermarks(&self) -> Result<(Vec<StoredWatermark>, i64), IndexerError> {
        // read_only transaction, otherwise this will block and get blocked by write
        // transactions to the same table.
        run_query_with_retry!(
            &self.blocking_cp,
            |conn| {
                let stored = watermarks::table
                    .load::<StoredWatermark>(conn)
                    .map_err(Into::into)
                    .context("Failed reading watermarks from PostgresDB")?;
                let timestamp = diesel::select(diesel::dsl::sql::<diesel::sql_types::BigInt>(
                    "(EXTRACT(EPOCH FROM CURRENT_TIMESTAMP) * 1000)::bigint",
                ))
                .get_result(conn)
                .map_err(Into::into)
                .context("Failed reading current timestamp from PostgresDB")?;
                Ok::<_, IndexerError>((stored, timestamp))
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )
    }

    async fn execute_in_blocking_worker<F, R>(&self, f: F) -> Result<R, IndexerError>
    where
        F: FnOnce(Self) -> Result<R, IndexerError> + Send + 'static,
        R: Send + 'static,
    {
        let this = self.clone();
        let current_span = tracing::Span::current();
        tokio::task::spawn_blocking(move || {
            mark_in_blocking_pool();
            let _guard = current_span.enter();
            f(this)
        })
        .await
        .map_err(Into::into)
        .and_then(std::convert::identity)
    }

    fn spawn_blocking_task<F, R>(
        &self,
        f: F,
    ) -> tokio::task::JoinHandle<std::result::Result<R, IndexerError>>
    where
        F: FnOnce(Self) -> Result<R, IndexerError> + Send + 'static,
        R: Send + 'static,
    {
        let this = self.clone();
        let current_span = tracing::Span::current();
        let guard = self.metrics.tokio_blocking_task_wait_latency.start_timer();
        tokio::task::spawn_blocking(move || {
            mark_in_blocking_pool();
            let _guard = current_span.enter();
            let _elapsed = guard.stop_and_record();
            f(this)
        })
    }

    fn spawn_task<F, Fut, R>(&self, f: F) -> tokio::task::JoinHandle<Result<R, IndexerError>>
    where
        F: FnOnce(Self) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<R, IndexerError>> + Send + 'static,
        R: Send + 'static,
    {
        let this = self.clone();
        tokio::task::spawn(async move { f(this).await })
    }
}

#[async_trait]
impl IndexerStore for PgIndexerStore {
    async fn get_latest_checkpoint_sequence_number(&self) -> Result<Option<u64>, IndexerError> {
        self.execute_in_blocking_worker(|this| this.get_latest_checkpoint_sequence_number())
            .await
    }

    async fn get_available_epoch_range(&self) -> Result<(u64, u64), IndexerError> {
        self.execute_in_blocking_worker(|this| this.get_prunable_epoch_range())
            .await
    }

    async fn get_available_checkpoint_range(&self) -> Result<(u64, u64), IndexerError> {
        self.execute_in_blocking_worker(|this| this.get_available_checkpoint_range())
            .await
    }

    async fn get_chain_identifier(&self) -> Result<Option<Vec<u8>>, IndexerError> {
        self.execute_in_blocking_worker(|this| this.get_chain_identifier())
            .await
    }

    async fn get_latest_object_snapshot_watermark(
        &self,
    ) -> Result<Option<CommitterWatermark>, IndexerError> {
        self.execute_in_blocking_worker(|this| this.get_latest_object_snapshot_watermark())
            .await
    }

    async fn get_latest_object_snapshot_checkpoint_sequence_number(
        &self,
    ) -> Result<Option<CheckpointSequenceNumber>, IndexerError> {
        self.execute_in_blocking_worker(|this| {
            this.get_latest_object_snapshot_checkpoint_sequence_number()
        })
        .await
    }

    fn persist_objects_in_existing_transaction(
        &self,
        conn: &mut PgConnection,
        object_changes: Vec<TransactionObjectChangesToCommit>,
    ) -> Result<(), IndexerError> {
        if object_changes.is_empty() {
            return Ok(());
        }

        let (indexed_mutations, indexed_deletions) = retain_latest_indexed_objects(object_changes);
        let object_mutations = indexed_mutations
            .into_iter()
            .map(StoredObject::from)
            .collect::<Vec<_>>();
        let object_deletions = indexed_deletions
            .into_iter()
            .map(StoredDeletedObject::from)
            .collect::<Vec<_>>();

        self.persist_object_mutation_chunk_in_existing_transaction(conn, object_mutations)?;
        self.persist_object_deletion_chunk_in_existing_transaction(conn, object_deletions)?;

        Ok(())
    }

    async fn persist_objects_snapshot(
        &self,
        object_changes: Vec<TransactionObjectChangesToCommit>,
    ) -> Result<(), IndexerError> {
        if object_changes.is_empty() {
            return Ok(());
        }
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_objects_snapshot
            .start_timer();
        let (indexed_mutations, indexed_deletions) = retain_latest_indexed_objects(object_changes);
        let objects_snapshot = indexed_mutations
            .into_iter()
            .map(StoredObjectSnapshot::from)
            .chain(
                indexed_deletions
                    .into_iter()
                    .map(StoredObjectSnapshot::from),
            )
            .collect::<Vec<_>>();
        let len = objects_snapshot.len();
        let chunks = chunk!(objects_snapshot, self.config.parallel_objects_chunk_size);
        let futures = chunks
            .into_iter()
            .map(|c| self.spawn_blocking_task(move |this| this.backfill_objects_snapshot_chunk(c)));

        futures::future::try_join_all(futures)
            .await
            .map_err(|e| {
                tracing::error!("failed to join backfill_objects_snapshot_chunk futures: {e}");
                IndexerError::from(e)
            })?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                IndexerError::PostgresWrite(format!(
                    "Failed to persist all objects snapshot chunks: {e:?}"
                ))
            })?;
        let elapsed = guard.stop_and_record();
        info!(elapsed, "Persisted {} objects snapshot", len);
        Ok(())
    }

    async fn persist_object_history(
        &self,
        object_changes: Vec<TransactionObjectChangesToCommit>,
    ) -> Result<(), IndexerError> {
        let skip_history = std::env::var("SKIP_OBJECT_HISTORY")
            .map(|val| val.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        if skip_history {
            info!("skipping object history");
            return Ok(());
        }

        if object_changes.is_empty() {
            return Ok(());
        }
        let objects = make_objects_history_to_commit(object_changes);
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_objects_history
            .start_timer();

        let len = objects.len();
        let chunks = chunk!(objects, self.config.parallel_objects_chunk_size);
        let futures = chunks
            .into_iter()
            .map(|c| self.spawn_blocking_task(move |this| this.persist_objects_history_chunk(c)));

        futures::future::try_join_all(futures)
            .await
            .map_err(|e| {
                tracing::error!("failed to join persist_objects_history_chunk futures: {e}");
                IndexerError::from(e)
            })?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                IndexerError::PostgresWrite(format!(
                    "Failed to persist all objects history chunks: {e:?}"
                ))
            })?;
        let elapsed = guard.stop_and_record();
        info!(elapsed, "Persisted {} objects history", len);
        Ok(())
    }

    async fn persist_object_versions(
        &self,
        object_versions: Vec<StoredObjectVersion>,
    ) -> Result<(), IndexerError> {
        if object_versions.is_empty() {
            return Ok(());
        }

        let guard = self
            .metrics
            .checkpoint_db_commit_latency_objects_version
            .start_timer();

        let object_versions_count = object_versions.len();

        let chunks = chunk!(object_versions, self.config.parallel_objects_chunk_size);
        let futures = chunks
            .into_iter()
            .map(|c| self.spawn_blocking_task(move |this| this.persist_object_version_chunk(c)))
            .collect::<Vec<_>>();

        futures::future::try_join_all(futures)
            .await
            .map_err(|e| {
                tracing::error!("failed to join persist_object_version_chunk futures: {e}");
                IndexerError::from(e)
            })?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                IndexerError::PostgresWrite(format!(
                    "Failed to persist all objects version chunks: {e:?}"
                ))
            })?;
        let elapsed = guard.stop_and_record();
        info!(elapsed, "Persisted {object_versions_count} object versions");
        Ok(())
    }

    async fn persist_checkpoints(
        &self,
        checkpoints: Vec<IndexedCheckpoint>,
    ) -> Result<(), IndexerError> {
        self.execute_in_blocking_worker(move |this| this.persist_checkpoints(checkpoints))
            .await
    }

    async fn persist_transactions(
        &self,
        transactions: Vec<IndexedTransaction>,
    ) -> Result<(), IndexerError> {
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_transactions
            .start_timer();
        let len = transactions.len();

        let chunks = chunk!(transactions, self.config.parallel_chunk_size);
        let futures = chunks
            .into_iter()
            .map(|c| self.spawn_blocking_task(move |this| this.persist_transactions_chunk(c)));

        futures::future::try_join_all(futures)
            .await
            .map_err(|e| {
                tracing::error!("failed to join persist_transactions_chunk futures: {e}");
                IndexerError::from(e)
            })?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                IndexerError::PostgresWrite(format!(
                    "Failed to persist all transactions chunks: {e:?}"
                ))
            })?;
        let elapsed = guard.stop_and_record();
        info!(elapsed, "Persisted {} transactions", len);
        Ok(())
    }

    fn persist_optimistic_transaction_in_existing_transaction(
        &self,
        conn: &mut PgConnection,
        transaction: OptimisticTransaction,
    ) -> Result<(), IndexerError> {
        insert_or_ignore_into!(optimistic_transactions::table, &transaction, conn);
        Ok(())
    }

    async fn persist_events(&self, events: Vec<IndexedEvent>) -> Result<(), IndexerError> {
        if events.is_empty() {
            return Ok(());
        }
        let len = events.len();
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_events
            .start_timer();
        let chunks = chunk!(events, self.config.parallel_chunk_size);
        let futures = chunks
            .into_iter()
            .map(|c| self.spawn_blocking_task(move |this| this.persist_events_chunk(c)));

        futures::future::try_join_all(futures)
            .await
            .map_err(|e| {
                tracing::error!("failed to join persist_events_chunk futures: {e}");
                IndexerError::from(e)
            })?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                IndexerError::PostgresWrite(format!("Failed to persist all events chunks: {e:?}"))
            })?;
        let elapsed = guard.stop_and_record();
        info!(elapsed, "Persisted {} events", len);
        Ok(())
    }

    async fn persist_displays(
        &self,
        display_updates: BTreeMap<String, StoredDisplay>,
    ) -> Result<(), IndexerError> {
        if display_updates.is_empty() {
            return Ok(());
        }

        self.spawn_blocking_task(move |this| this.persist_display_updates(display_updates))
            .await?
    }

    fn persist_displays_in_existing_transaction(
        &self,
        conn: &mut PgConnection,
        display_updates: Vec<&StoredDisplay>,
    ) -> Result<(), IndexerError> {
        if display_updates.is_empty() {
            return Ok(());
        }

        on_conflict_do_update_with_condition!(
            display::table,
            display_updates,
            display::object_type,
            (
                display::id.eq(excluded(display::id)),
                display::version.eq(excluded(display::version)),
                display::bcs.eq(excluded(display::bcs)),
            ),
            excluded(display::version).gt(display::version),
            conn
        );

        Ok(())
    }

    async fn persist_packages(&self, packages: Vec<IndexedPackage>) -> Result<(), IndexerError> {
        if packages.is_empty() {
            return Ok(());
        }
        self.execute_in_blocking_worker(move |this| this.persist_packages(packages))
            .await
    }

    async fn persist_event_indices(&self, indices: Vec<EventIndex>) -> Result<(), IndexerError> {
        if indices.is_empty() {
            return Ok(());
        }
        let len = indices.len();
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_event_indices
            .start_timer();
        let chunks = chunk!(indices, self.config.parallel_chunk_size);

        let futures = chunks.into_iter().map(|chunk| {
            self.spawn_task(move |this: Self| async move {
                this.persist_event_indices_chunk(chunk).await
            })
        });

        futures::future::try_join_all(futures)
            .await
            .map_err(|e| {
                tracing::error!("failed to join persist_event_indices_chunk futures: {e}");
                IndexerError::from(e)
            })?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                IndexerError::PostgresWrite(format!(
                    "Failed to persist all event_indices chunks: {e:?}"
                ))
            })?;
        let elapsed = guard.stop_and_record();
        info!(elapsed, "Persisted {} event_indices chunks", len);
        Ok(())
    }

    async fn persist_epoch(&self, epoch: EpochToCommit) -> Result<(), IndexerError> {
        self.execute_in_blocking_worker(move |this| this.persist_epoch(epoch))
            .await
    }

    async fn advance_epoch(&self, epoch: EpochToCommit) -> Result<(), IndexerError> {
        self.execute_in_blocking_worker(move |this| this.advance_epoch(epoch))
            .await
    }

    async fn prune_epoch(&self, epoch: u64) -> Result<(), IndexerError> {
        let (mut min_cp, max_cp) = match self.get_checkpoint_range_for_epoch(epoch)? {
            (min_cp, Some(max_cp)) => Ok((min_cp, max_cp)),
            _ => Err(IndexerError::PostgresRead(format!(
                "Failed to get checkpoint range for epoch {epoch}"
            ))),
        }?;

        // NOTE: for disaster recovery, min_cp is the min cp of the current epoch, which
        // is likely partially pruned already. min_prunable_cp is the min cp to
        // be pruned. By std::cmp::max, we will resume the pruning process from
        // the next checkpoint, instead of the first cp of the current epoch.
        let min_prunable_cp = self.get_min_prunable_checkpoint()?;
        min_cp = std::cmp::max(min_cp, min_prunable_cp);
        for cp in min_cp..=max_cp {
            // NOTE: the order of pruning tables is crucial:
            // 1. prune checkpoints table, checkpoints table is the source table of
            //    available range,
            // we prune it first to make sure that we always have full data for checkpoints
            // within the available range;
            // 2. then prune tx_* tables;
            // 3. then prune pruner_cp_watermark table, which is the checkpoint pruning
            //    watermark table and also tx seq source
            // of a checkpoint to prune tx_* tables;
            // 4. lastly we prune epochs table when all checkpoints of the epoch have been
            //    pruned.
            info!(
                "Pruning checkpoint {} of epoch {} (min_prunable_cp: {})",
                cp, epoch, min_prunable_cp
            );
            self.execute_in_blocking_worker(move |this| this.prune_checkpoints_table(cp))
                .await
                .unwrap_or_else(|e| {
                    tracing::error!("failed to prune checkpoint {cp}: {e}");
                });

            let (min_tx, max_tx) = self.get_transaction_range_for_checkpoint(cp)?;
            self.execute_in_blocking_worker(move |this| {
                this.prune_tx_indices_table(min_tx, max_tx)
            })
            .await
            .unwrap_or_else(|e| {
                tracing::error!("failed to prune transactions for cp {cp}: {e}");
            });
            info!(
                "Pruned transactions for checkpoint {} from tx {} to tx {}",
                cp, min_tx, max_tx
            );
            self.execute_in_blocking_worker(move |this| {
                this.prune_event_indices_table(min_tx, max_tx)
            })
            .await
            .unwrap_or_else(|e| {
                tracing::error!("failed to prune events of transactions for cp {cp}: {e}");
            });
            info!(
                "Pruned events of transactions for checkpoint {cp} from tx {min_tx} to tx {max_tx}"
            );
            self.metrics.last_pruned_transaction.set(max_tx as i64);

            self.execute_in_blocking_worker(move |this| this.prune_cp_tx_table(cp))
                .await
                .unwrap_or_else(|e| {
                    tracing::error!("failed to prune pruner_cp_watermark table for cp {cp}: {e}");
                });
            info!("Pruned checkpoint {} of epoch {}", cp, epoch);
            self.metrics.last_pruned_checkpoint.set(cp as i64);
        }

        Ok(())
    }

    async fn get_network_total_transactions_by_end_of_epoch(
        &self,
        epoch: u64,
    ) -> Result<Option<u64>, IndexerError> {
        self.execute_in_blocking_worker(move |this| {
            this.get_network_total_transactions_by_end_of_epoch(epoch)
        })
        .await
    }

    async fn refresh_participation_metrics(&self) -> Result<(), IndexerError> {
        self.execute_in_blocking_worker(move |this| this.refresh_participation_metrics())
            .await
    }

    async fn update_watermarks_upper_bound<E: IntoEnumIterator>(
        &self,
        watermark: CommitterWatermark,
    ) -> Result<(), IndexerError>
    where
        E::Iterator: Iterator<Item: AsRef<str>>,
    {
        self.execute_in_blocking_worker(move |this| {
            this.update_watermarks_upper_bound::<E>(watermark)
        })
        .await
    }

    fn as_any(&self) -> &dyn StdAny {
        self
    }

    /// Persist protocol configs and feature flags until the protocol version
    /// for the latest epoch we have stored in the db, inclusive.
    fn persist_protocol_configs_and_feature_flags(
        &self,
        chain_id: Vec<u8>,
    ) -> Result<(), IndexerError> {
        let chain_id = ChainIdentifier::from(
            CheckpointDigest::try_from(chain_id).expect("unable to convert chain id"),
        );

        let mut all_configs = vec![];
        let mut all_flags = vec![];

        let (start_version, end_version) = self.get_protocol_version_index_range()?;
        info!(
            "Persisting protocol configs with start_version: {}, end_version: {}",
            start_version, end_version
        );

        // Gather all protocol configs and feature flags for all versions between start
        // and end.
        for version in start_version..=end_version {
            let protocol_configs = ProtocolConfig::get_for_version_if_supported(
                (version as u64).into(),
                chain_id.chain(),
            )
            .ok_or(IndexerError::Generic(format!(
                "Unable to fetch protocol version {} and chain {:?}",
                version,
                chain_id.chain()
            )))?;
            let configs_vec = protocol_configs
                .attr_map()
                .into_iter()
                .map(|(k, v)| StoredProtocolConfig {
                    protocol_version: version,
                    config_name: k,
                    config_value: v.map(|v| v.to_string()),
                })
                .collect::<Vec<_>>();
            all_configs.extend(configs_vec);

            let feature_flags = protocol_configs
                .feature_map()
                .into_iter()
                .map(|(k, v)| StoredFeatureFlag {
                    protocol_version: version,
                    flag_name: k,
                    flag_value: v,
                })
                .collect::<Vec<_>>();
            all_flags.extend(feature_flags);
        }

        transactional_blocking_with_retry!(
            &self.blocking_cp,
            |conn| {
                for config_chunk in all_configs.chunks(PG_COMMIT_CHUNK_SIZE_INTRA_DB_TX) {
                    insert_or_ignore_into!(protocol_configs::table, config_chunk, conn);
                }
                insert_or_ignore_into!(feature_flags::table, all_flags.clone(), conn);
                Ok::<(), IndexerError>(())
            },
            PG_DB_COMMIT_SLEEP_DURATION
        )?;
        Ok(())
    }

    async fn persist_tx_indices(&self, indices: Vec<TxIndex>) -> Result<(), IndexerError> {
        if indices.is_empty() {
            return Ok(());
        }
        let len = indices.len();
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_tx_indices
            .start_timer();
        let chunks = chunk!(indices, self.config.parallel_chunk_size);

        let futures = chunks.into_iter().map(|chunk| {
            self.spawn_task(move |this: Self| async move {
                this.persist_tx_indices_chunk_v2(chunk).await
            })
        });
        futures::future::try_join_all(futures)
            .await
            .map_err(|e| {
                tracing::error!("failed to join persist_tx_indices_chunk futures: {e}");
                IndexerError::from(e)
            })?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                IndexerError::PostgresWrite(format!(
                    "Failed to persist all tx_indices chunks: {e:?}"
                ))
            })?;
        let elapsed = guard.stop_and_record();
        info!(elapsed, "Persisted {} tx_indices chunks", len);
        Ok(())
    }

    async fn persist_checkpoint_objects(
        &self,
        objects: Vec<CheckpointObjectChanges>,
    ) -> Result<(), IndexerError> {
        if objects.is_empty() {
            return Ok(());
        }
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_objects
            .start_timer();
        let CheckpointObjectChanges {
            changed_objects: mutations,
            deleted_objects: deletions,
        } = retain_latest_objects_from_checkpoint_batch(objects);
        let mutation_len = mutations.len();
        let deletion_len = deletions.len();

        let mutation_chunks = chunk!(mutations, self.config.parallel_objects_chunk_size);
        let deletion_chunks = chunk!(deletions, self.config.parallel_objects_chunk_size);
        let mutation_futures = mutation_chunks
            .into_iter()
            .map(|c| self.spawn_blocking_task(move |this| this.persist_changed_objects(c)));
        let deletion_futures = deletion_chunks
            .into_iter()
            .map(|c| self.spawn_blocking_task(move |this| this.persist_removed_objects(c)));
        futures::future::try_join_all(mutation_futures.chain(deletion_futures))
            .await
            .map_err(|e| {
                tracing::error!("failed to join futures for persisting objects: {e}");
                IndexerError::from(e)
            })?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                IndexerError::PostgresWrite(format!("Failed to persist all object chunks: {e:?}",))
            })?;

        let elapsed = guard.stop_and_record();
        info!(
            elapsed,
            "Persisted objects with {mutation_len} mutations and {deletion_len} deletions",
        );
        Ok(())
    }

    async fn update_status_for_checkpoint_transactions(
        &self,
        tx_order: Vec<CheckpointTxGlobalOrder>,
    ) -> Result<(), IndexerError> {
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_tx_insertion_order
            .start_timer();
        let len = tx_order.len();

        let chunks = chunk!(tx_order, self.config.parallel_chunk_size);
        let futures = chunks.into_iter().map(|c| {
            self.spawn_blocking_task(move |this| {
                this.update_status_for_checkpoint_transactions_chunk(c)
            })
        });

        futures::future::try_join_all(futures)
            .await
            .map_err(|e| {
                tracing::error!(
                    "failed to join update_status_for_checkpoint_transactions_chunk futures: {e}",
                );
                IndexerError::from(e)
            })?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                IndexerError::PostgresWrite(format!(
                    "Failed to update all `tx_global_order` chunks: {e:?}",
                ))
            })?;
        let elapsed = guard.stop_and_record();
        info!(
            elapsed,
            "Updated index status for {len} txs insertion orders"
        );
        Ok(())
    }

    async fn persist_tx_global_order(
        &self,
        tx_order: Vec<CheckpointTxGlobalOrder>,
    ) -> Result<(), IndexerError> {
        let guard = self
            .metrics
            .checkpoint_db_commit_latency_tx_insertion_order
            .start_timer();
        let len = tx_order.len();

        let chunks = chunk!(tx_order, self.config.parallel_chunk_size);
        let futures = chunks
            .into_iter()
            .map(|c| self.spawn_blocking_task(move |this| this.persist_tx_global_order_chunk(c)));

        futures::future::try_join_all(futures)
            .await
            .map_err(|e| {
                tracing::error!("failed to join persist_tx_global_order_chunk futures: {e}",);
                IndexerError::from(e)
            })?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                IndexerError::PostgresWrite(format!(
                    "Failed to persist all txs insertion order chunks: {e:?}",
                ))
            })?;
        let elapsed = guard.stop_and_record();
        info!(elapsed, "Persisted {len} txs insertion orders");
        Ok(())
    }

    async fn update_watermarks_lower_bound(
        &self,
        watermarks: Vec<(PrunableTable, u64)>,
    ) -> Result<(), IndexerError> {
        self.execute_in_blocking_worker(move |this| this.update_watermarks_lower_bound(watermarks))
            .await
    }

    async fn get_watermarks(&self) -> Result<(Vec<StoredWatermark>, i64), IndexerError> {
        self.execute_in_blocking_worker(move |this| this.get_watermarks())
            .await
    }
}

fn make_objects_history_to_commit(
    tx_object_changes: Vec<TransactionObjectChangesToCommit>,
) -> Vec<StoredHistoryObject> {
    let deleted_objects: Vec<StoredHistoryObject> = tx_object_changes
        .clone()
        .into_iter()
        .flat_map(|changes| changes.deleted_objects)
        .map(|o| o.into())
        .collect();
    let mutated_objects: Vec<StoredHistoryObject> = tx_object_changes
        .into_iter()
        .flat_map(|changes| changes.changed_objects)
        .map(|o| o.into())
        .collect();
    deleted_objects.into_iter().chain(mutated_objects).collect()
}

/// Partitions object changes into deletions and mutations.
///
/// Retains only the highest version of each object among deletions and
/// mutations. This allows concurrent insertion into the DB of the resulting
/// partitions.
fn retain_latest_indexed_objects(
    tx_object_changes: Vec<TransactionObjectChangesToCommit>,
) -> (Vec<IndexedObject>, Vec<IndexedDeletedObject>) {
    use std::collections::HashMap;

    let mut mutations = HashMap::<ObjectID, IndexedObject>::new();
    let mut deletions = HashMap::<ObjectID, IndexedDeletedObject>::new();

    for change in tx_object_changes {
        // Remove mutation / deletion with a following deletion / mutation,
        // as we expect that following deletion / mutation has a higher version.
        // Technically, assertions below are not required, double check just in case.
        for mutation in change.changed_objects {
            let id = mutation.object.id();
            let version = mutation.object.version();

            if let Some(existing) = deletions.remove(&id) {
                assert!(
                    existing.object_version < version.value(),
                    "mutation version ({version:?}) should be greater than existing deletion version ({:?}) for object {id:?}",
                    existing.object_version
                );
            }

            if let Some(existing) = mutations.insert(id, mutation) {
                assert!(
                    existing.object.version() < version,
                    "mutation version ({version:?}) should be greater than existing mutation version ({:?}) for object {id:?}",
                    existing.object.version()
                );
            }
        }
        // Handle deleted objects
        for deletion in change.deleted_objects {
            let id = deletion.object_id;
            let version = deletion.object_version;

            if let Some(existing) = mutations.remove(&id) {
                assert!(
                    existing.object.version().value() < version,
                    "deletion version ({version:?}) should be greater than existing mutation version ({:?}) for object {id:?}",
                    existing.object.version(),
                );
            }

            if let Some(existing) = deletions.insert(id, deletion) {
                assert!(
                    existing.object_version < version,
                    "deletion version ({version:?}) should be greater than existing deletion version ({:?}) for object {id:?}",
                    existing.object_version
                );
            }
        }
    }

    (
        mutations.into_values().collect(),
        deletions.into_values().collect(),
    )
}
