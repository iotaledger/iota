// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashMap},
    time::Duration,
};

use diesel::{
    PgConnection, QueryResult, QueryableByName, RunQueryDsl,
    sql_types::{BigInt, Text, VarChar},
};
use downcast::Any;
use tracing::{error, info};

use crate::{
    db::ConnectionPool,
    errors::IndexerError,
    models::epoch::{EpochLowerBounds, StartOfEpochUpdate, StoredEpochInfo},
    store::diesel_macro::*,
};

const GET_PARTITION_SQL: &str = {
    r"
SELECT parent.relname                                            AS table_name,
       MIN(CAST(SUBSTRING(child.relname FROM '\d+$') AS BIGINT)) AS first_partition,
       MAX(CAST(SUBSTRING(child.relname FROM '\d+$') AS BIGINT)) AS last_partition
FROM pg_inherits
         JOIN pg_class parent ON pg_inherits.inhparent = parent.oid
         JOIN pg_class child ON pg_inherits.inhrelid = child.oid
         JOIN pg_namespace nmsp_parent ON nmsp_parent.oid = parent.relnamespace
         JOIN pg_namespace nmsp_child ON nmsp_child.oid = child.relnamespace
WHERE parent.relkind = 'p'
GROUP BY table_name;
"
};

pub struct PgPartitionManager {
    cp: ConnectionPool,
    partition_strategies: HashMap<&'static str, PgPartitionStrategy>,
}

impl Clone for PgPartitionManager {
    fn clone(&self) -> PgPartitionManager {
        Self {
            cp: self.cp.clone(),
            partition_strategies: self.partition_strategies.clone(),
        }
    }
}

#[derive(Clone, Copy)]
pub enum PgPartitionStrategy {
    Epoch(EpochPartitionKey),
    ObjectId,
}

/// Partition key of a table that has one partition for each epoch.
#[derive(Clone, Copy)]
pub enum EpochPartitionKey {
    CheckpointSequenceNumber,
    TxSequenceNumber,
}

impl PgPartitionStrategy {
    /// Returns the partition key of an epoch-partitioned table, or `None` for other strategies.
    pub fn epoch(&self) -> Option<EpochPartitionKey> {
        match self {
            PgPartitionStrategy::Epoch(key) => Some(*key),
            PgPartitionStrategy::ObjectId => None,
        }
    }

    pub fn is_epoch_partitioned(&self) -> bool {
        self.epoch().is_some()
    }
}

impl EpochPartitionKey {
    /// Returns the first partition key associated to `epoch`.
    pub fn start(&self, epoch: &EpochLowerBounds) -> i64 {
        match self {
            Self::CheckpointSequenceNumber => epoch.first_checkpoint_id,
            Self::TxSequenceNumber => epoch.first_tx_sequence_number,
        }
    }
}

#[derive(Clone, Debug)]
pub struct EpochPartitionData {
    last_epoch: u64,
    next_epoch: u64,
    last_epoch_start_cp: u64,
    next_epoch_start_cp: u64,
    last_epoch_start_tx: u64,
    next_epoch_start_tx: u64,
}

impl EpochPartitionData {
    pub fn compose_data(new_epoch: StartOfEpochUpdate, last_db_epoch: StoredEpochInfo) -> Self {
        let last_epoch = last_db_epoch.epoch as u64;
        let last_epoch_start_cp = last_db_epoch.first_checkpoint_id as u64;
        let next_epoch = new_epoch.epoch as u64;
        let next_epoch_start_cp = new_epoch.first_checkpoint_id as u64;

        let next_epoch_start_tx = new_epoch.first_tx_sequence_number as u64;
        let last_epoch_start_tx = last_db_epoch.first_tx_sequence_number as u64;

        Self {
            last_epoch,
            next_epoch,
            last_epoch_start_cp,
            next_epoch_start_cp,
            last_epoch_start_tx,
            next_epoch_start_tx,
        }
    }
}

impl PgPartitionManager {
    pub fn new(cp: ConnectionPool) -> Self {
        let mut partition_strategies = HashMap::new();
        partition_strategies.insert(
            "events",
            PgPartitionStrategy::Epoch(EpochPartitionKey::TxSequenceNumber),
        );
        partition_strategies.insert(
            "transactions",
            PgPartitionStrategy::Epoch(EpochPartitionKey::TxSequenceNumber),
        );
        partition_strategies.insert("objects_version", PgPartitionStrategy::ObjectId);
        Self {
            cp,
            partition_strategies,
        }
    }

    pub fn get_table_partitions(&self) -> Result<BTreeMap<String, (u64, u64)>, IndexerError> {
        #[derive(QueryableByName, Debug, Clone)]
        struct PartitionedTable {
            #[diesel(sql_type = VarChar)]
            table_name: String,
            #[diesel(sql_type = BigInt)]
            first_partition: i64,
            #[diesel(sql_type = BigInt)]
            last_partition: i64,
        }

        Ok(
            read_only_blocking!(&self.cp, |conn| diesel::RunQueryDsl::load(
                diesel::sql_query(GET_PARTITION_SQL),
                conn
            ))?
            .into_iter()
            .map(|table: PartitionedTable| {
                (
                    table.table_name,
                    (table.first_partition as u64, table.last_partition as u64),
                )
            })
            .collect(),
        )
    }

    /// Tries to fetch the partitioning strategy for the given partitioned
    /// table. Defaults to `CheckpointSequenceNumber` as the majority of our
    /// tables are partitioned on an epoch's checkpoints today.
    pub fn get_strategy(&self, table_name: &str) -> PgPartitionStrategy {
        self.partition_strategies
            .get(table_name)
            .copied()
            .unwrap_or(PgPartitionStrategy::Epoch(
                EpochPartitionKey::CheckpointSequenceNumber,
            ))
    }

    pub fn determine_epoch_partition_range(
        &self,
        table_name: &str,
        data: &EpochPartitionData,
    ) -> Option<(u64, u64)> {
        match self.get_strategy(table_name) {
            PgPartitionStrategy::Epoch(EpochPartitionKey::CheckpointSequenceNumber) => {
                Some((data.last_epoch_start_cp, data.next_epoch_start_cp))
            }
            PgPartitionStrategy::Epoch(EpochPartitionKey::TxSequenceNumber) => {
                Some((data.last_epoch_start_tx, data.next_epoch_start_tx))
            }
            PgPartitionStrategy::ObjectId => None,
        }
    }

    fn advance_partition(
        &self,
        table: &str,
        last_partition: u64,
        next_partition: u64,
        last_partition_start: u64,
        next_partition_start: u64,
    ) -> Result<(), IndexerError> {
        let last_partition = last_partition.try_into()?;
        let next_partition = next_partition.try_into()?;
        let last_partition_start = last_partition_start.try_into()?;
        let next_partition_start = next_partition_start.try_into()?;
        transactional_blocking_with_retry!(
            &self.cp,
            |conn| {
                call_alter_partition_range(
                    conn,
                    table,
                    last_partition,
                    last_partition_start,
                    next_partition_start,
                )?;
                call_create_partition(conn, table, next_partition, next_partition_start)
            },
            Duration::from_secs(10)
        )?;
        Ok(())
    }

    /// Changes the value range of `<table>_partition_<partition>` to
    /// `[start, end)`.
    ///
    /// The partition must exist, and all of its rows must be in the new range.
    ///
    /// # Errors
    ///
    /// Returns an error if the database rejects the detach or the attach of the
    /// partition.
    pub(crate) fn alter_partition_range(
        &self,
        table: &str,
        partition: i64,
        start: i64,
        end: i64,
    ) -> Result<(), IndexerError> {
        transactional_blocking_with_retry!(
            &self.cp,
            |conn| call_alter_partition_range(conn, table, partition, start, end),
            Duration::from_secs(10)
        )?;
        Ok(())
    }

    /// Creates `<table>_partition_<partition>` with the value range
    /// `[start, MAXVALUE)`.
    ///
    /// Does nothing if the partition exists.
    ///
    /// # Errors
    ///
    /// Returns an error if the database rejects the creation of the partition.
    pub(crate) fn create_partition(
        &self,
        table: &str,
        partition: i64,
        start: i64,
    ) -> Result<(), IndexerError> {
        transactional_blocking_with_retry!(
            &self.cp,
            |conn| call_create_partition(conn, table, partition, start),
            Duration::from_secs(10)
        )?;
        Ok(())
    }

    pub fn advance_epoch(
        &self,
        table: String,
        last_partition: u64,
        data: &EpochPartitionData,
    ) -> Result<(), IndexerError> {
        let Some((last_epoch_start, next_epoch_start)) =
            self.determine_epoch_partition_range(&table, data)
        else {
            return Ok(());
        };
        if data.next_epoch == 0 {
            info!("Epoch 0 partition has been created in the initial setup.");
            return Ok(());
        }
        if last_partition == data.last_epoch {
            self.advance_partition(
                &table,
                data.last_epoch,
                data.next_epoch,
                last_epoch_start,
                next_epoch_start,
            )?;
            info!(
                "Advanced epoch partition for table {} from {} to {}, prev partition upper bound {}",
                table, last_partition, data.next_epoch, next_epoch_start
            );
        } else if last_partition == 0 {
            // this applies to instances that were restored from a formal snapshot before
            // v1.31
            info!("Healing epoch-based partitions. This might take some time.");
            self.advance_partition(&table, last_partition, data.next_epoch, 0, next_epoch_start)?;
            info!(
                "Healed epoch partition for table {} from {} to {}, prev partition upper bound {}",
                table, last_partition, data.next_epoch, next_epoch_start
            );
        } else if last_partition != data.next_epoch {
            error!(
                "Advancing to epoch {} failed. Corrupted partitions for table {table}",
                data.last_epoch
            );
            return Err(IndexerError::PostgresWrite);
        } else {
            info!(
                "Epoch has been advanced to {} already, skipping.",
                data.next_epoch
            );
        }
        Ok(())
    }

    pub fn drop_table_partition(&self, table: String, partition: u64) -> Result<(), IndexerError> {
        transactional_blocking_with_retry!(
            &self.cp,
            |conn| {
                RunQueryDsl::execute(
                    diesel::sql_query("CALL drop_partition($1, $2)")
                        .bind::<diesel::sql_types::Text, _>(table.clone())
                        .bind::<diesel::sql_types::BigInt, _>(partition as i64),
                    conn,
                )
            },
            Duration::from_secs(10)
        )?;
        Ok(())
    }
}

fn call_alter_partition_range(
    conn: &mut PgConnection,
    table: &str,
    partition: i64,
    start: i64,
    end: i64,
) -> QueryResult<usize> {
    diesel::sql_query("CALL alter_partition_range($1, $2, $3, $4)")
        .bind::<Text, _>(table)
        .bind::<BigInt, _>(partition)
        .bind::<BigInt, _>(start)
        .bind::<BigInt, _>(end)
        .execute(conn)
}

fn call_create_partition(
    conn: &mut PgConnection,
    table: &str,
    partition: i64,
    start: i64,
) -> QueryResult<usize> {
    diesel::sql_query("CALL create_partition($1, $2, $3)")
        .bind::<Text, _>(table)
        .bind::<BigInt, _>(partition)
        .bind::<BigInt, _>(start)
        .execute(conn)
}
