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

#[derive(Subcommand, Clone, Debug)]
#[non_exhaustive]
pub enum BackfillTaskKind {
    /// \sql is the SQL string to run, appended with the range between the start
    /// and end, as well as conflict resolution (see sql_backfill.rs).
    /// \key_column is the primary key column to use for the range.
    Sql { sql: String, key_column: String },
    /// Starts a backfill pipeline from the ingestion engine.
    /// \remote_store_url is the URL of the remote store to ingest from.
    /// Any `IngestionBackfillKind` will need to map to a type that
    /// implements `IngestionBackfillTrait`.
    Ingestion {
        kind: IngestionBackfillKind,
        remote_store_url: String,
    },
}

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
            unimplemented!("No backfill tasks for ingestion backfills implemented yet.")
        }
    }
}
