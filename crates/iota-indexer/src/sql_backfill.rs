// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::{Duration, Instant};

use diesel::{prelude::*, sql_types::BigInt};
use downcast::Any;
use futures::stream::{self, StreamExt, TryStreamExt};
use tap::TapFallible;
use tracing::{error, info, instrument};

use crate::{db::ConnectionPool, errors::IndexerError, transactional_blocking_with_retry};

const DEFAULT_CHUNK_SIZE: u64 = 10000;
const DEFAULT_MAX_CONCURRENCY: usize = 100;

/// A backfiller that runs SQL queries in parallel to update a range of rows in
/// a database table.
pub struct SqlBackfiller {
    pool: ConnectionPool,
    base_sql: String,
    checkpoint_col: String,
    chunk_size: u64,
    max_concurrency: usize,
}

impl SqlBackfiller {
    /// Construct a new backfiller with default concurrency settings
    pub fn new<S: Into<String>>(pool: ConnectionPool, base_sql: S, checkpoint_col: S) -> Self {
        Self {
            pool,
            base_sql: base_sql.into(),
            checkpoint_col: checkpoint_col.into(),
            chunk_size: DEFAULT_CHUNK_SIZE,
            max_concurrency: DEFAULT_MAX_CONCURRENCY,
        }
    }

    /// Run the backfill between `first` and `last` checkpoint, in parallel
    #[instrument(skip(self), fields(first = first, last = last))]
    pub async fn run(&self, first: u64, last: u64) -> Result<(), IndexerError> {
        let timer = Instant::now();

        let ranges = (first..=last)
            .step_by(self.chunk_size as usize)
            .map(|start_id| {
                let end_id = std::cmp::min(start_id + self.chunk_size - 1, last);
                (start_id, end_id)
            });

        stream::iter(ranges)
            .map(|range| async move { self.backfill_range(range).await })
            .buffer_unordered(self.max_concurrency)
            .try_for_each(|((start, end), rows)| async move {
                info!(start, end, rows, "Completed backfill chunk");
                Ok(())
            })
            .await?;

        info!(elapsed = ?timer.elapsed(), "Finished full backfill");
        Ok(())
    }

    /// Backfill a single (start, end) range in a blocking task
    #[instrument(skip(self))]
    async fn backfill_range(
        &self,
        (start, end): (u64, u64),
    ) -> Result<((u64, u64), usize), IndexerError> {
        let query_sql = format!(
            "{} WHERE {} BETWEEN $1 AND $2",
            self.base_sql, self.checkpoint_col
        );
        let query_clone = query_sql.clone();

        let rows = transactional_blocking_with_retry!(
            &self.pool,
            |conn| {
                let affected = diesel::sql_query(&query_clone)
                    .bind::<BigInt, _>(start as i64)
                    .bind::<BigInt, _>(end as i64)
                    .execute(conn)?;
                Ok::<usize, IndexerError>(affected)
            },
            Duration::from_secs(3600) // Retry delay
        )
        .tap_ok(|len| {
            info!("Persisted {len} chunks");
        })
        .tap_err(|e| {
            error!("Failed to persist object mutations with error: {}", e);
        })?;

        Ok(((start, end), rows))
    }
}

#[cfg(test)]
mod tests {

    fn database_url(db_name: &str) -> String {
        format!("postgres://postgres:postgrespw@localhost:5432/{db_name}")
    }

    use std::time::Duration;

    use diesel::{
        prelude::*,
        sql_query,
        sql_types::{BigInt, Bool},
    };

    use super::SqlBackfiller;
    use crate::{db::ConnectionPool, errors::IndexerError, test_utils::TestDatabase};

    #[derive(QueryableByName, Debug)]
    struct ProcessCounts {
        #[sql_type = "BigInt"]
        processed: i64,
        #[sql_type = "BigInt"]
        unprocessed: i64,
    }

    // Helper function to set up a test table with some initial data
    fn setup_test_table(pool: &ConnectionPool) {
        let mut conn = pool.get().unwrap();
        sql_query(
            r#"
            CREATE TABLE test_items (
                id BIGINT PRIMARY KEY,
                processed BOOLEAN NOT NULL DEFAULT FALSE
            )
        "#,
        )
        .execute(&mut conn)
        .unwrap();

        for id in 1..=20 {
            sql_query("INSERT INTO test_items (id) VALUES ($1)")
                .bind::<BigInt, _>(id)
                .execute(&mut conn)
                .unwrap();
        }
    }

    #[tokio::test]
    async fn backfill_marks_processed() -> Result<(), IndexerError> {
        let mut database = TestDatabase::new(database_url("backfill_marks_processed"));
        database.recreate();
        database.reset_db();

        {
            let pool: ConnectionPool = database.to_connection_pool();
            setup_test_table(&pool);

            let mut backfiller =
                SqlBackfiller::new(pool.clone(), "UPDATE test_items SET processed = TRUE", "id");
            backfiller.chunk_size = 10;
            backfiller.max_concurrency = 2;

            // Backfill for IDs 5..=15
            backfiller.run(5, 15).await?;

            let mut conn = pool.get().unwrap();
            let counts: ProcessCounts = sql_query(
                r#"
            SELECT
              SUM(CASE WHEN processed THEN 1 ELSE 0 END)   AS processed,
              SUM(CASE WHEN NOT processed THEN 1 ELSE 0 END) AS unprocessed
            FROM test_items
        "#,
            )
            .get_result(&mut conn)
            .unwrap();

            assert_eq!(counts.processed, 11, "Should have processed 11 items");
            assert_eq!(counts.unprocessed, 9, "Should have 9 unprocessed items");
        }

        database.drop_if_exists();
        Ok(())
    }
}
