// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use clap::Subcommand;

pub mod backfill_instances;
pub mod backfill_runner;
pub mod backfill_task;

#[derive(Subcommand, Clone, Debug)]
#[non_exhaustive]
pub enum BackfillTaskKind {
    /// \sql is the SQL string to run, appended with the range between the start
    /// and end, as well as conflict resolution (see sql_backfill.rs).
    /// \key_column is the primary key column to use for the range.
    Sql { sql: String, key_column: String },
}
