// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Types and logic to setup and maintain the database.
//!
//! Creating connections, applying or validating migrations are examples of
//! operations included in this scope.

use std::{collections::HashSet, time::Duration};

use anyhow::anyhow;
use clap::Args;
use diesel::{
    PgConnection, QueryableByName,
    connection::BoxableConnection,
    query_dsl::RunQueryDsl,
    r2d2::{ConnectionManager, Pool, PooledConnection, R2D2Connection},
};
use strum::IntoEnumIterator;
use tracing::info;

use crate::{errors::IndexerError, pruning::pruner::PrunableTable};

pub type ConnectionPool = Pool<ConnectionManager<PgConnection>>;
pub type PoolConnection = PooledConnection<ConnectionManager<PgConnection>>;

#[derive(Args, Debug, Clone)]
pub struct ConnectionPoolConfig {
    #[arg(long, default_value_t = 100)]
    #[arg(env = "DB_POOL_SIZE")]
    pub pool_size: u32,
    #[arg(long, value_parser = parse_duration, default_value = "30")]
    #[arg(env = "DB_CONNECTION_TIMEOUT")]
    pub connection_timeout: Duration,
    #[arg(long, value_parser = parse_duration, default_value = "3600")]
    #[arg(env = "DB_STATEMENT_TIMEOUT")]
    pub statement_timeout: Duration,
}

fn parse_duration(arg: &str) -> Result<std::time::Duration, std::num::ParseIntError> {
    let seconds = arg.parse()?;
    Ok(std::time::Duration::from_secs(seconds))
}

impl ConnectionPoolConfig {
    pub const DEFAULT_POOL_SIZE: u32 = 100;
    pub const DEFAULT_CONNECTION_TIMEOUT: u64 = 30;
    pub const DEFAULT_STATEMENT_TIMEOUT: u64 = 3600;

    fn connection_config(&self) -> ConnectionConfig {
        ConnectionConfig {
            statement_timeout: self.statement_timeout,
            read_only: false,
        }
    }

    pub fn set_pool_size(&mut self, size: u32) {
        self.pool_size = size;
    }

    pub fn set_connection_timeout(&mut self, timeout: Duration) {
        self.connection_timeout = timeout;
    }

    pub fn set_statement_timeout(&mut self, timeout: Duration) {
        self.statement_timeout = timeout;
    }
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            pool_size: Self::DEFAULT_POOL_SIZE,
            connection_timeout: Duration::from_secs(Self::DEFAULT_CONNECTION_TIMEOUT),
            statement_timeout: Duration::from_secs(Self::DEFAULT_STATEMENT_TIMEOUT),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ConnectionConfig {
    pub statement_timeout: Duration,
    pub read_only: bool,
}

impl<T: R2D2Connection + 'static> diesel::r2d2::CustomizeConnection<T, diesel::r2d2::Error>
    for ConnectionConfig
{
    fn on_acquire(&self, _conn: &mut T) -> std::result::Result<(), diesel::r2d2::Error> {
        _conn
            .as_any_mut()
            .downcast_mut::<diesel::PgConnection>()
            .map_or_else(
                || {
                    Err(diesel::r2d2::Error::QueryError(
                        diesel::result::Error::DeserializationError(
                            "failed to downcast connection to PgConnection"
                                .to_string()
                                .into(),
                        ),
                    ))
                },
                |pg_conn| {
                    diesel::sql_query(format!(
                        "SET statement_timeout = {}",
                        self.statement_timeout.as_millis(),
                    ))
                    .execute(pg_conn)
                    .map_err(diesel::r2d2::Error::QueryError)?;

                    if self.read_only {
                        diesel::sql_query("SET default_transaction_read_only = 't'")
                            .execute(pg_conn)
                            .map_err(diesel::r2d2::Error::QueryError)?;
                    }
                    Ok(())
                },
            )?;
        Ok(())
    }
}

pub fn new_connection_pool(
    db_url: &str,
    config: &ConnectionPoolConfig,
) -> Result<ConnectionPool, IndexerError> {
    let manager = ConnectionManager::<PgConnection>::new(db_url);

    Pool::builder()
        .max_size(config.pool_size)
        .connection_timeout(config.connection_timeout)
        .connection_customizer(Box::new(config.connection_config()))
        .build(manager)
        .map_err(|e| {
            IndexerError::PgConnectionPoolInit(format!(
                "failed to initialize connection pool for {db_url} with error: {e:?}"
            ))
        })
}

pub fn get_pool_connection(pool: &ConnectionPool) -> Result<PoolConnection, IndexerError> {
    pool.get().map_err(|e| {
        IndexerError::PgPoolConnection(format!(
            "failed to get connection from PG connection pool with error: {e:?}"
        ))
    })
}

pub fn reset_database(conn: &mut PoolConnection) -> Result<(), anyhow::Error> {
    {
        conn.as_any_mut()
            .downcast_mut::<PoolConnection>()
            .map_or_else(
                || Err(anyhow!("failed to downcast connection to PgConnection")),
                |pg_conn| {
                    setup_postgres::reset_database(pg_conn)?;
                    Ok(())
                },
            )?;
    }
    Ok(())
}

/// Check that prunable tables exist in the database.
pub async fn check_prunable_tables_valid(conn: &mut PoolConnection) -> Result<(), IndexerError> {
    info!("Starting compatibility check");

    use diesel::RunQueryDsl;

    let select_parent_tables = r#"
    SELECT c.relname AS table_name
    FROM pg_class c
    JOIN pg_namespace n ON n.oid = c.relnamespace
    LEFT JOIN pg_partitioned_table pt ON pt.partrelid = c.oid
    WHERE c.relkind IN ('r', 'p')  -- 'r' for regular tables, 'p' for partitioned tables
        AND n.nspname = 'public'
        AND (
            pt.partrelid IS NOT NULL  -- This is a partitioned (parent) table
            OR NOT EXISTS (  -- This is not a partition (child table)
                SELECT 1
                FROM pg_inherits i
                WHERE i.inhrelid = c.oid
            )
        );
    "#;

    #[derive(QueryableByName)]
    struct TableName {
        #[diesel(sql_type = diesel::sql_types::Text)]
        table_name: String,
    }

    let result: Vec<TableName> = diesel::sql_query(select_parent_tables)
        .load(conn)
        .map_err(|e| IndexerError::DbMigration(format!("failed to fetch tables: {e}")))?;

    let parent_tables_from_db: HashSet<_> = result.into_iter().map(|t| t.table_name).collect();

    for key in PrunableTable::iter() {
        if !parent_tables_from_db.contains(key.as_ref()) {
            return Err(IndexerError::Generic(format!(
                "invalid retention policy override provided for table {key}: does not exist in the database",
            )));
        }
    }

    info!("Compatibility check passed");
    Ok(())
}

pub mod setup_postgres {
    use anyhow::anyhow;
    use diesel::{
        RunQueryDsl,
        migration::{Migration, MigrationConnection, MigrationSource, MigrationVersion},
        pg::Pg,
        prelude::*,
    };
    use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
    use tracing::info;

    use crate::{IndexerError, db::PoolConnection};

    table! {
        __diesel_schema_migrations (version) {
            version -> VarChar,
            run_on -> Timestamp,
        }
    }

    const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/pg");

    pub fn reset_database(conn: &mut PoolConnection) -> Result<(), anyhow::Error> {
        info!("Resetting PG database ...");

        let drop_all_tables = "
        DO $$ DECLARE
            r RECORD;
        BEGIN
        FOR r IN (SELECT tablename FROM pg_tables WHERE schemaname = 'public')
            LOOP
                EXECUTE 'DROP TABLE IF EXISTS ' || quote_ident(r.tablename) || ' CASCADE';
            END LOOP;
        END $$;";
        diesel::sql_query(drop_all_tables).execute(conn)?;
        info!("Dropped all tables.");

        let drop_all_procedures = "
        DO $$ DECLARE
            r RECORD;
        BEGIN
            FOR r IN (SELECT proname, oidvectortypes(proargtypes) as argtypes
                      FROM pg_proc INNER JOIN pg_namespace ns ON (pg_proc.pronamespace = ns.oid)
                      WHERE ns.nspname = 'public' AND prokind = 'p')
            LOOP
                EXECUTE 'DROP PROCEDURE IF EXISTS ' || quote_ident(r.proname) || '(' || r.argtypes || ') CASCADE';
            END LOOP;
        END $$;";
        diesel::sql_query(drop_all_procedures).execute(conn)?;
        info!("Dropped all procedures.");

        let drop_all_functions = "
        DO $$ DECLARE
            r RECORD;
        BEGIN
            FOR r IN (SELECT proname, oidvectortypes(proargtypes) as argtypes
                      FROM pg_proc INNER JOIN pg_namespace ON (pg_proc.pronamespace = pg_namespace.oid)
                      WHERE pg_namespace.nspname = 'public' AND prokind = 'f')
            LOOP
                EXECUTE 'DROP FUNCTION IF EXISTS ' || quote_ident(r.proname) || '(' || r.argtypes || ') CASCADE';
            END LOOP;
        END $$;";
        diesel::sql_query(drop_all_functions).execute(conn)?;
        info!("Dropped all functions.");

        conn.setup()?;
        info!("Created __diesel_schema_migrations table.");

        run_migrations(conn)?;
        info!("Reset database complete.");
        Ok(())
    }

    /// Execute all unapplied migrations.
    pub fn run_migrations(conn: &mut PoolConnection) -> Result<(), anyhow::Error> {
        let pending_migrations = conn
            .pending_migrations(MIGRATIONS)
            .map_err(|e| anyhow!("failed to identify pending migrations {e}"))?;
        for migration in pending_migrations {
            info!("Applying migration {}", migration.name());
            conn.run_migration(&migration)
                .map_err(|e| anyhow!("failed to run migration {e}"))?;
        }
        Ok(())
    }

    /// Checks that the local migration scripts are a prefix of the records in
    /// the database. This allows to run migration scripts against a DB at
    /// any time, without worrying about existing readers failing over.
    ///
    /// # Deployment Requirement
    /// Whenever deploying a new version of either the reader or writer,
    /// migration scripts **must** be run first. This ensures that there are
    /// never more local migration scripts than those recorded in the database.
    ///
    /// # Backward Compatibility
    /// All new migrations must be **backward compatible** with the previous
    /// data model. Do **not** remove or rename columns, tables, or change types
    /// in a way that would break older versions of the reader or writer.
    ///
    /// Only after all services are running the new code and no old versions
    /// are in use, can you safely remove deprecated fields or make breaking
    /// changes.
    ///
    /// This approach supports rolling upgrades and prevents unnecessary
    /// failures during deployment.
    pub fn check_db_migration_consistency(conn: &mut PoolConnection) -> Result<(), IndexerError> {
        info!("Starting compatibility check");
        let migrations: Vec<Box<dyn Migration<Pg>>> = MIGRATIONS.migrations().map_err(|err| {
            IndexerError::DbMigration(format!(
                "failed to fetch local migrations from schema: {err}"
            ))
        })?;

        let local_migrations = migrations
            .iter()
            .map(|m| m.name().version())
            .collect::<Vec<_>>();

        check_db_migration_consistency_impl(conn, local_migrations)?;
        info!("Compatibility check passed");
        Ok(())
    }

    fn check_db_migration_consistency_impl(
        conn: &mut PoolConnection,
        local_migrations: Vec<MigrationVersion>,
    ) -> Result<(), IndexerError> {
        // Unfortunately we cannot call applied_migrations() directly on the connection,
        // since it implicitly creates the __diesel_schema_migrations table if it
        // doesn't exist, which is a write operation that we don't want to do in
        // this function.
        let applied_migrations: Vec<MigrationVersion> = __diesel_schema_migrations::table
            .select(__diesel_schema_migrations::version)
            .order(__diesel_schema_migrations::version.asc())
            .load(conn)?;

        // We check that the local migrations is a prefix of the applied migrations.
        if local_migrations.len() > applied_migrations.len() {
            return Err(IndexerError::DbMigration(format!(
                "the number of local migrations is greater than the number of applied migrations. Local migrations: {local_migrations:?}, Applied migrations: {applied_migrations:?}",
            )));
        }
        for (local_migration, applied_migration) in local_migrations.iter().zip(&applied_migrations)
        {
            if local_migration != applied_migration {
                return Err(IndexerError::DbMigration(format!(
                    "the next applied migration `{applied_migration:?}` diverges from the local migration `{local_migration:?}`",
                )));
            }
        }
        Ok(())
    }

    #[cfg(feature = "pg_integration")]
    #[cfg(test)]
    mod tests {
        use diesel::{
            migration::{Migration, MigrationSource},
            pg::Pg,
        };
        use diesel_migrations::MigrationHarness;

        use crate::{
            db::setup_postgres::{self, MIGRATIONS},
            test_utils::{TestDatabase, db_url},
        };

        // Check that the migration records in the database created from the local
        // schema pass the consistency check.
        #[test]
        fn db_migration_consistency_smoke_test() {
            let mut database = TestDatabase::new(db_url("db_migration_consistency_smoke_test"));
            database.recreate();
            database.reset_db();
            {
                let pool = database.to_connection_pool();
                let mut conn = pool.get().unwrap();
                setup_postgres::check_db_migration_consistency(&mut conn).unwrap();
            }
            database.drop_if_exists();
        }

        #[test]
        fn db_migration_consistency_non_prefix_test() {
            let mut database =
                TestDatabase::new(db_url("db_migration_consistency_non_prefix_test"));
            database.recreate();
            database.reset_db();
            {
                let pool = database.to_connection_pool();
                let mut conn = pool.get().unwrap();
                conn.revert_migration(MIGRATIONS.migrations().unwrap().last().unwrap())
                    .unwrap();
                // Local migrations is one record more than the applied migrations.
                // This will fail the consistency check since it's not a prefix.
                assert!(setup_postgres::check_db_migration_consistency(&mut conn).is_err());

                conn.run_pending_migrations(MIGRATIONS).unwrap();
                // After running pending migrations they should be consistent.
                setup_postgres::check_db_migration_consistency(&mut conn).unwrap();
            }
            database.drop_if_exists();
        }

        #[test]
        fn db_migration_consistency_prefix_test() {
            let mut database = TestDatabase::new(db_url("db_migration_consistency_prefix_test"));
            database.recreate();
            database.reset_db();
            {
                let pool = database.to_connection_pool();
                let mut conn = pool.get().unwrap();

                let migrations: Vec<Box<dyn Migration<Pg>>> = MIGRATIONS.migrations().unwrap();
                let mut local_migrations: Vec<_> =
                    migrations.iter().map(|m| m.name().version()).collect();
                local_migrations.pop();
                // Local migrations is one record less than the applied migrations.
                // This should pass the consistency check since it's still a prefix.
                setup_postgres::check_db_migration_consistency_impl(&mut conn, local_migrations)
                    .unwrap();
            }
            database.drop_if_exists();
        }
    }
}
