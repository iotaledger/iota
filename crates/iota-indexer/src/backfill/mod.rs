// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use clap::{Subcommand, ValueEnum};

use crate::backfill::{sql::task::SqlBackfillTask, task::BackfillTask};

pub(crate) mod ingestion;
pub mod runner;
pub(crate) mod sql;
pub(crate) mod task;

// Subcommands for selecting a backfill task to run.
// Each variant corresponds to a different backfill implementation.
#[derive(Subcommand, Clone, Debug)]
#[non_exhaustive]
pub enum BackfillTaskKind {
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
    /// - `remote_store_url`: the endpoint or path of the remote checkpoint
    ///   store to ingest from.
    ///
    /// The runner will spawn the data ingestion workflow, continuously buffer
    /// processed checkpoint data, and then slice the requested checkpoint
    /// range into chunks for database backfill.
    Ingestion {
        kind: IngestionBackfillKind,
        remote_store_url: String,
    },
}

/// Selects the concrete ingestion backfill task to run.
/// Each variant of `IngestionBackfillKind` must correspond to a type that
/// implements the `IngestionBackfill` trait.
#[derive(ValueEnum, Clone, Debug)]
#[non_exhaustive]
pub enum IngestionBackfillKind {}

pub(crate) async fn get_backfill_task(
    kind: BackfillTaskKind,
    _range_start: usize,
) -> Arc<dyn BackfillTask> {
    match kind {
        BackfillTaskKind::Sql { sql, key_column } => {
            Arc::new(SqlBackfillTask::new(sql, key_column))
        }
        BackfillTaskKind::Ingestion { .. } => {
            unimplemented!("No ingestion backfill tasks implemented yet.")
        }
    }
}

#[cfg(test)]
mod test_utils {
    use diesel::{QueryableByName, sql_types::BigInt};

    /// Returns the database URL for testing purposes.
    pub(crate) fn database_url(db_name: &str) -> String {
        format!("postgres://postgres:postgrespw@localhost:5432/{db_name}")
    }

    /// Counts how many rows exist in the target table.
    #[derive(QueryableByName, Debug)]
    pub(crate) struct RowCount {
        #[diesel(sql_type = BigInt)]
        pub(crate) cnt: i64,
    }
}
