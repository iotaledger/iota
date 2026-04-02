// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{ops::RangeInclusive, sync::Arc};

use async_trait::async_trait;
use clap::{Subcommand, ValueEnum};
use iota_types::messages_checkpoint::CheckpointSequenceNumber;

use crate::{
    backfill::{
        ingestion::{
            jobs::{
                object_changes_unwrapped::ObjectChangesUnwrappedBackfill,
                tx_wrapped_or_deleted_objects::TxWrappedOrDeletedObjectsBackfill,
            },
            task::IngestionBackfillTask,
        },
        sql::sql_backfill::SqlBackfill,
    },
    config::IngestionConfig,
    db::ConnectionPool,
    errors::IndexerError,
};

pub(crate) mod ingestion;
pub mod runner;
pub(crate) mod sql;

/// Encapsulates the logic to fetch, process, and persist data for a given
/// numeric range.
///
/// The provided `range` is an inclusive numeric interval specifying the subset
/// of data to handle. Specific semantics of the numeric values (e.g.,
/// checkpoint sequence numbers, transaction sequence numbers, or other ordered
/// identifiers) depend on each implementation.
#[async_trait]
pub(crate) trait Backfill: Send + Sync {
    async fn backfill_range(
        &self,
        pool: ConnectionPool,
        range: &RangeInclusive<usize>,
    ) -> Result<(), IndexerError>;
}

/// Subcommands for selecting a backfill task to run.
/// Each variant corresponds to a different backfill implementation.
#[derive(Subcommand, Clone, Debug)]
#[non_exhaustive]
pub enum BackfillKind {
    /// Run a SQL backfill.
    ///
    /// - `sql`: the base SQL statement to execute (without any `WHERE` clause).
    ///   For each chunk `[start, end]`, the tool will append: ```sql WHERE
    ///   {key_column} BETWEEN {start} AND {end} ``` and automatically handle
    ///   conflict resolution by adding `ON CONFLICT DO NOTHING`.
    /// - `key_column`: the name of the column to filter on, typically a
    ///   sequence number primary key.
    Sql { sql: String, key_column: String },
    /// Run a backfill driven by the ingestion engine.
    ///
    /// - `kind`: defines the specific ingestion backfill implementation to use.
    /// - `config`: configuration options for the data ingestion worker.
    ///
    /// The runner will spawn the data ingestion workflow, continuously buffer
    /// processed checkpoint data, and then slice the requested checkpoint
    /// range into chunks for database backfill.
    Ingestion {
        kind: IngestionBackfillKind,
        #[command(flatten)]
        config: Box<IngestionConfig>,
    },
}

/// Selects the concrete ingestion backfill task to run.
/// Each variant of `IngestionBackfillKind` must correspond to a type that
/// implements the `IngestionBackfill` trait.
#[derive(ValueEnum, Clone, Debug)]
#[non_exhaustive]
pub enum IngestionBackfillKind {
    /// Backfills the `tx_wrapped_or_deleted_objects` table.
    TxWrappedOrDeletedObjects,
    ObjectChangesUnwrapped,
}

pub(crate) async fn get_backfill(
    kind: BackfillKind,
    range_start: usize,
) -> Result<Arc<dyn Backfill>, IndexerError> {
    match kind {
        BackfillKind::Sql { sql, key_column } => Ok(Arc::new(SqlBackfill::new(sql, key_column))),
        BackfillKind::Ingestion { kind, config } => match kind {
            IngestionBackfillKind::TxWrappedOrDeletedObjects => Ok(Arc::new(
                IngestionBackfillTask::<TxWrappedOrDeletedObjectsBackfill>::new(
                    *config,
                    range_start as CheckpointSequenceNumber,
                )
                .await?,
            )),
            IngestionBackfillKind::ObjectChangesUnwrapped => Ok(Arc::new(
                IngestionBackfillTask::<ObjectChangesUnwrappedBackfill>::new(
                    *config,
                    range_start as CheckpointSequenceNumber,
                )
                .await?,
            )),
        },
    }
}
