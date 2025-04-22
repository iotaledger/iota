// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use anyhow::anyhow;
use clap::Args;
use diesel::{
    PgConnection,
    connection::BoxableConnection,
    query_dsl::RunQueryDsl,
    r2d2::{ConnectionManager, Pool, PooledConnection, R2D2Connection},
};

use crate::errors::IndexerError;

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
                            "Failed to downcast connection to PgConnection"
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
                "Failed to initialize connection pool for {db_url} with error: {e:?}"
            ))
        })
}

pub fn get_pool_connection(pool: &ConnectionPool) -> Result<PoolConnection, IndexerError> {
    pool.get().map_err(|e| {
        IndexerError::PgPoolConnection(format!(
            "Failed to get connection from PG connection pool with error: {:?}",
            e
        ))
    })
}

pub fn reset_database(conn: &mut PoolConnection) -> Result<(), anyhow::Error> {
    {
        conn.as_any_mut()
            .downcast_mut::<PoolConnection>()
            .map_or_else(
                || Err(anyhow!("Failed to downcast connection to PgConnection")),
                |pg_conn| {
                    setup_postgres::reset_database(pg_conn)?;
                    Ok(())
                },
            )?;
    }
    Ok(())
}

pub mod setup_postgres {
    use anyhow::anyhow;
    use diesel::{RunQueryDsl, migration::MigrationSource};
    use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
    use tracing::info;

    use crate::db::PoolConnection;

    pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/pg");

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

        diesel::sql_query(
            "
        CREATE TABLE IF NOT EXISTS __diesel_schema_migrations (
            version VARCHAR(50) PRIMARY KEY,
            run_on TIMESTAMP NOT NULL DEFAULT NOW()
        )",
        )
        .execute(conn)?;
        info!("Created __diesel_schema_migrations table.");

        conn.run_migrations(&MIGRATIONS.migrations().unwrap())
            .map_err(|e| anyhow!("Failed to run migrations {e}"))?;
        info!("Reset database complete.");
        Ok(())
    }
}
