// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::{Duration, Instant};

use clap::Args;
use diesel::{prelude::*, sql_types::BigInt};
use downcast::Any;
use futures::stream::{self, StreamExt, TryStreamExt};
use tap::TapFallible;
use tracing::{error, info, instrument};

use crate::{db::ConnectionPool, errors::IndexerError, transactional_blocking_with_retry};

const DEFAULT_CHUNK_SIZE: u64 = 10000;
const DEFAULT_MAX_CONCURRENCY: usize = 100;
const BACKFILL_RETRY_INTERVAL: Duration = Duration::from_secs(30);

/// Configuration for the SQL backfill operation.
#[derive(Args, Debug, Clone)]
pub struct SqlBackfillerConfig {
    /// Base SQL statement **without** the `WHERE` clause.
    /// Example: `"INSERT INTO full_objects_history (id, data) SELECT id, data
    /// FROM objects_history"`.
    #[clap(long, env = "SQL_BACKFILL_BASE_SQL")]
    pub base_sql: String,
    /// Name of the checkpoint column used for chunk‐based filtering.
    /// Example: `"checkpoint_sequence_number"`.
    #[clap(long, env = "SQL_BACKFILL_CHECKPOINT_COL")]
    pub checkpoint_col: String,
    /// Number of rows (by checkpoint value) to process per chunk.
    /// Default: `10000`.
    #[clap(long, default_value_t = DEFAULT_CHUNK_SIZE, env = "SQL_BACKFILL_CHUNK_SIZE")]
    pub chunk_size: u64,
    /// Maximum number of concurrent backfill tasks.
    /// Default: `100`.
    #[clap(long, default_value_t = DEFAULT_MAX_CONCURRENCY, env = "SQL_BACKFILL_MAX_CONCURRENCY")]
    pub max_concurrency: usize,
    /// Skip rows whose primary key already exists (i.e. append `ON CONFLICT DO
    /// NOTHING`).
    #[clap(
        long = "skip-existing",
        env = "SQL_BACKFILL_SKIP_EXISTING",
        help = "Append `ON CONFLICT DO NOTHING` to each chunked `INSERT`"
    )]
    pub skip_existing: bool,
}

/// A backfiller that runs SQL queries in parallel to update a range of rows in
/// a database table.
pub struct SqlBackfiller {
    pool: ConnectionPool,
    config: SqlBackfillerConfig,
}

impl SqlBackfiller {
    /// Creates a new `SqlBackfiller` with the given connection pool and
    /// configuration.
    ///
    /// Validates that:
    /// - `base_sql` is not empty.
    /// - `checkpoint_col` is not empty.
    ///
    /// Any other SQL errors—such as syntax issues, non-INSERT statements,
    /// or missing tables—will be reported by Postgres at runtime.
    pub fn new(pool: ConnectionPool, config: SqlBackfillerConfig) -> Result<Self, IndexerError> {
        let base_sql = config.base_sql.trim();
        if base_sql.is_empty() {
            return Err(IndexerError::InvalidArgument(
                "base_sql must not be empty".into(),
            ));
        }

        if config.checkpoint_col.trim().is_empty() {
            return Err(IndexerError::InvalidArgument(
                "checkpoint_col must not be empty".into(),
            ));
        }

        Ok(Self { pool, config })
    }

    /// Run the backfill between `first` and `last` checkpoint.
    #[instrument(skip(self), fields(first = first, last = last))]
    pub async fn run(&self, first: u64, last: u64) -> Result<(), IndexerError> {
        info!(
            first = first,
            last = last,
            chunk_size = self.config.chunk_size,
            max_concurrency = self.config.max_concurrency,
            skip_existing = self.config.skip_existing,
            "Starting backfill",
        );

        let timer = Instant::now();

        let chunk_size = self.config.chunk_size;
        let ranges = (first..=last).step_by(chunk_size as usize).map(|start_id| {
            let end_id = std::cmp::min(start_id + chunk_size - 1, last);
            (start_id, end_id)
        });

        stream::iter(ranges)
            .map(|range| async move { self.backfill_range(range).await })
            .buffer_unordered(self.config.max_concurrency)
            .try_for_each(|((start, end), rows)| async move {
                info!(start, end, rows, "Completed backfill chunk");
                Ok(())
            })
            .await?;

        info!(elapsed = ?timer.elapsed(), "Finished backfill");
        Ok(())
    }

    /// Backfill a single (start, end) range.
    #[instrument(skip(self))]
    async fn backfill_range(
        &self,
        (start, end): (u64, u64),
    ) -> Result<((u64, u64), usize), IndexerError> {
        let query_sql = self.build_sql();

        let rows = transactional_blocking_with_retry!(
            &self.pool,
            |conn| {
                let affected = diesel::sql_query(&query_sql)
                    .bind::<BigInt, _>(start as i64)
                    .bind::<BigInt, _>(end as i64)
                    .execute(conn)?;
                Ok::<usize, IndexerError>(affected)
            },
            BACKFILL_RETRY_INTERVAL
        )
        .tap_ok(|len| {
            info!("Persisted {len} backfill rows for range {start}..={end}");
        })
        .tap_err(|e| {
            error!("Failed to persist backfill rows for range {start}..={end}: {e}");
        })?;

        Ok(((start, end), rows))
    }

    fn build_sql(&self) -> String {
        let mut sql = format!(
            "{} WHERE {} BETWEEN $1 AND $2",
            self.config.base_sql, self.config.checkpoint_col
        );

        if self.config.skip_existing {
            sql.push_str(" ON CONFLICT DO NOTHING");
        }

        sql
    }
}

#[cfg(test)]
mod tests {
    use diesel::sql_query;

    use super::*;
    use crate::test_utils::TestDatabase;

    fn database_url(db_name: &str) -> String {
        format!("postgres://postgres:postgrespw@localhost:5432/{db_name}")
    }

    /// Counts how many rows exist in the target table.
    #[derive(QueryableByName, Debug)]
    struct RowCount {
        #[diesel(sql_type = BigInt)]
        cnt: i64,
    }

    fn setup_source_and_target(pool: &ConnectionPool) {
        let mut conn = pool.get().unwrap();

        // Create source_items
        sql_query(
            r#"
        CREATE TABLE source_items (
            id BIGINT PRIMARY KEY,
            payload TEXT NOT NULL
        )
        "#,
        )
        .execute(&mut conn)
        .unwrap();

        // Populate source_items
        sql_query(
            r#"INSERT INTO source_items (id, payload)
           SELECT generate_series(1,20), 'data'"#,
        )
        .execute(&mut conn)
        .unwrap();

        // Create target_items
        sql_query(
            r#"
        CREATE TABLE target_items (
            id BIGINT PRIMARY KEY,
            payload TEXT NOT NULL
        )
        "#,
        )
        .execute(&mut conn)
        .unwrap();

        // Seed target_items with 1..=10
        sql_query(
            r#"INSERT INTO target_items (id, payload)
           SELECT generate_series(1,10), 'data'"#,
        )
        .execute(&mut conn)
        .unwrap();

        // Seed target_items with 16..=20
        sql_query(
            r#"INSERT INTO target_items (id, payload)
           SELECT generate_series(16,20), 'data'"#,
        )
        .execute(&mut conn)
        .unwrap();
    }

    #[tokio::test]
    async fn insert_gap_fills_missing_ids() -> Result<(), IndexerError> {
        telemetry_subscribers::init_for_testing();

        let mut database = TestDatabase::new(database_url("insert_gap_filler"));
        database.recreate();
        database.reset_db();

        {
            let pool: ConnectionPool = database.to_connection_pool();
            setup_source_and_target(&pool);

            // Only insert the missing IDs 11..=15
            let config = SqlBackfillerConfig {
                base_sql: "INSERT INTO target_items (id, payload)\
                   SELECT id, payload FROM source_items"
                    .into(),
                checkpoint_col: "id".into(),
                chunk_size: 5,
                max_concurrency: 2,
                skip_existing: false,
            };

            let backfiller = SqlBackfiller::new(pool.clone(), config)?;
            backfiller.run(11, 15).await?;

            let mut conn = pool.get().unwrap();
            let RowCount { cnt } = sql_query("SELECT COUNT(*) AS cnt FROM target_items")
                .get_result(&mut conn)
                .unwrap();

            // Initially 10 + 5 + 5 = 20 total
            assert_eq!(cnt, 20, "Should have filled exactly 5 missing rows");
        }

        database.drop_if_exists();
        Ok(())
    }

    #[tokio::test]
    async fn skip_duplicates_allows_retry() -> Result<(), IndexerError> {
        telemetry_subscribers::init_for_testing();

        let mut database = TestDatabase::new(database_url("skip_duplicates_retry"));
        database.recreate();
        database.reset_db();

        {
            let pool: ConnectionPool = database.to_connection_pool();
            setup_source_and_target(&pool);

            // Use skip_duplicates = true
            let config = SqlBackfillerConfig {
                base_sql:
                    "INSERT INTO target_items (id, payload) SELECT id, payload FROM source_items"
                        .into(),
                checkpoint_col: "id".into(),
                chunk_size: 2,
                max_concurrency: 4,
                skip_existing: true,
            };
            let backfiller = SqlBackfiller::new(pool.clone(), config)?;

            // First run fills missing IDs 11..=13
            backfiller.run(11, 13).await?;

            // Rerun with ID 13 being already present. Will be skipped,
            // so only IDs 14 and 15 get inserted.
            backfiller.run(13, 15).await?;

            // Verify total count remains correct (20 rows)
            let mut conn = pool.get().unwrap();
            let RowCount { cnt } = sql_query("SELECT COUNT(*) AS cnt FROM target_items")
                .get_result(&mut conn)
                .unwrap();
            assert_eq!(cnt, 20, "Count should remain at 20 after retry");
        }

        database.drop_if_exists();
        Ok(())
    }
}
