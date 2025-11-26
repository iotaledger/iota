// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::ops::RangeInclusive;

use async_trait::async_trait;
use diesel::{RunQueryDsl, sql_types::BigInt};

use crate::{
    backfill::Backfill,
    db::{ConnectionPool, get_pool_connection},
    errors::IndexerError,
};

/// A backfiller that runs SQL queries in parallel to update a range of rows in
/// a database table.
pub(crate) struct SqlBackfill {
    sql: String,
    key_column: String,
}

impl SqlBackfill {
    /// Creates a new `SqlBackfill` instance with the provided SQL query and
    /// key column.
    pub fn new(sql: String, key_column: String) -> Self {
        Self { sql, key_column }
    }
}

#[async_trait]
impl Backfill for SqlBackfill {
    async fn backfill_range(
        &self,
        pool: ConnectionPool,
        range: &RangeInclusive<usize>,
    ) -> Result<(), IndexerError> {
        let start = *range.start();
        let end = *range.end();

        let query = format!(
            "{} WHERE {} BETWEEN $1 AND $2 ON CONFLICT DO NOTHING",
            self.sql, self.key_column,
        );

        let mut conn = get_pool_connection(&pool)?;

        diesel::sql_query(&query)
            .bind::<BigInt, _>(start as i64)
            .bind::<BigInt, _>(end as i64)
            .execute(&mut conn)?;

        Ok(())
    }
}

#[cfg(feature = "pg_integration")]
#[cfg(test)]
mod tests {
    use diesel::sql_query;

    use super::*;
    use crate::{
        backfill::{BackfillKind, runner::BackfillRunner},
        config::BackfillConfig,
        test_utils::{RowCount, TestDatabase, db_url},
    };

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

        let mut database = TestDatabase::new(db_url("insert_gap_filler"));
        database.recreate();
        database.reset_db();

        {
            let pool: ConnectionPool = database.to_connection_pool();
            setup_source_and_target(&pool);

            let backfill_config = BackfillConfig {
                chunk_size: 5,
                max_concurrency: 2,
            };

            let total_range = 11..=15;

            BackfillRunner::run(
                BackfillKind::Sql {
                    sql: "INSERT INTO target_items (id, payload) SELECT id, payload FROM source_items"
                        .into(),
                    key_column: "id".into(),
                },
                pool.clone(),
                backfill_config,
                total_range,
            )
                .await?;

            let mut conn = pool.get().unwrap();
            let RowCount { cnt } = sql_query("SELECT COUNT(*) AS cnt FROM target_items")
                .get_result(&mut conn)
                .unwrap();

            assert_eq!(cnt, 20, "should have filled exactly 5 missing rows");
        }

        database.drop_if_exists();
        Ok(())
    }

    #[tokio::test]
    async fn skip_duplicates_allows_retry() -> Result<(), IndexerError> {
        telemetry_subscribers::init_for_testing();

        let mut database = TestDatabase::new(db_url("skip_duplicates_retry"));
        database.recreate();
        database.reset_db();

        {
            let pool: ConnectionPool = database.to_connection_pool();
            setup_source_and_target(&pool);

            let backfill_config = BackfillConfig {
                chunk_size: 2,
                max_concurrency: 4,
            };

            // First run fills missing IDs 11..=13
            BackfillRunner::run(
                BackfillKind::Sql {
                    sql: "INSERT INTO target_items (id, payload) SELECT id, payload FROM source_items"
                        .into(),
                    key_column: "id".into(),
                },
                pool.clone(),
                backfill_config.clone(),
                11..=13,
            )
                .await?;

            // Rerun overlaps at ID 13, should fill IDs 14 and 15 only
            BackfillRunner::run(
                BackfillKind::Sql {
                    sql: "INSERT INTO target_items (id, payload) SELECT id, payload FROM source_items"
                        .into(),
                    key_column: "id".into(),
                },
                pool.clone(),
                backfill_config,
                13..=15,
            )
                .await?;

            let mut conn = pool.get().unwrap();
            let RowCount { cnt } = sql_query("SELECT COUNT(*) AS cnt FROM target_items")
                .get_result(&mut conn)
                .unwrap();
            assert_eq!(cnt, 20, "count should remain at 20 after retry");
        }

        database.drop_if_exists();
        Ok(())
    }
}
