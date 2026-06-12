// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeSet, HashSet},
    sync::Arc,
};

use async_trait::async_trait;
use iota_data_ingestion_core::Worker;
use iota_sdk_types::Owner;
use iota_types::{
    base_types::IotaAddress, effects::TransactionEffectsExt,
    full_checkpoint_content::CheckpointData, transaction::TransactionDataAPI,
};
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

use crate::{BigTableClient, KeyValueStoreWriter, TransactionData};

/// Represents the BigTable tables used by the KvWorker.

// Variants are declared in write order; the BTreeSet<Table> field
// iterates by derived Ord, which follows declaration order.
//
// Order matters: data tables (Objects, Transactions, Checkpoints)
// are written before their indexes (TransactionsByAddress,
// CheckpointsByDigest), so a reader that sees an index entry can
// always resolve the underlying row. Reordering variants will
// reorder writes and may break that read-after-write invariant.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    strum::EnumIter,
)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Table {
    Objects,
    Transactions,
    TransactionsByAddress,
    Checkpoints,
    CheckpointsByDigest,
}

/// This worker implementation is responsible for processing checkpoints by
/// storing its data as Key-Value pairs. The Key-Value pairs are stored in a
/// BigTableDB.
pub struct KvWorker {
    pub client: BigTableClient,
    /// The tables enabled for writing by this worker.
    pub enabled_tables: BTreeSet<Table>,
}

impl KvWorker {
    /// Creates a new KvWorker that writes data to all tables.
    ///
    /// For selective writing, use [`KvWorker::new_selective`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use iota_kvstore::KvWorker;
    /// # use iota_kvstore::{Table, BigTableClient};
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// # std::env::set_var("BIGTABLE_EMULATOR_HOST", "localhost");
    /// let client = BigTableClient::new_local("instance_id", "column_family")
    ///     .await
    ///     .unwrap();
    ///
    /// /// Write all available tables to BigTable.
    /// let worker = KvWorker::new(client);
    ///
    /// # drop(worker);
    /// # }
    /// ```
    pub fn new(client: BigTableClient) -> Self {
        Self {
            client,
            // All tables are enabled by default.
            enabled_tables: Table::iter().collect(),
        }
    }

    /// Creates a new KvWorker that writes only to the specified tables.
    ///
    /// # NOTE
    /// Passing an empty iterator yields a worker that performs no writes.
    /// Use [`KvWorker::new`] to write to all tables.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use iota_kvstore::KvWorker;
    /// # use iota_kvstore::{Table, BigTableClient};
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// # std::env::set_var("BIGTABLE_EMULATOR_HOST", "localhost");
    /// let client = BigTableClient::new_local("instance_id", "column_family")
    ///     .await
    ///     .unwrap();
    ///
    /// /// Write only the `Objects` table to BigTable.
    /// let worker = KvWorker::new_selective(client, [Table::Objects]);
    ///
    /// # drop(worker);
    /// # }
    /// ```
    pub fn new_selective(client: BigTableClient, tables: impl IntoIterator<Item = Table>) -> Self {
        Self {
            client,
            enabled_tables: BTreeSet::from_iter(tables),
        }
    }
}

#[async_trait]
impl Worker for KvWorker {
    type Message = ();
    type Error = anyhow::Error;

    async fn process_checkpoint(&self, checkpoint: Arc<CheckpointData>) -> anyhow::Result<()> {
        let mut client = self.client.clone();

        for table in &self.enabled_tables {
            match table {
                Table::Objects => {
                    let objects = checkpoint
                        .transactions
                        .iter()
                        .flat_map(|t| &t.output_objects)
                        .collect::<Vec<_>>();
                    client.save_objects(&objects).await?;
                }
                Table::Transactions => {
                    let transactions = checkpoint
                        .transactions
                        .iter()
                        .map(|t| {
                            TransactionData::new(t, checkpoint.checkpoint_summary.sequence_number)
                        })
                        .collect::<Vec<_>>();
                    client.save_transactions(&transactions).await?;
                }
                Table::TransactionsByAddress => {
                    let entries_by_address = checkpoint
                        .checkpoint_contents
                        .enumerate_transactions(&checkpoint.checkpoint_summary)
                        .zip(&checkpoint.transactions)
                        .flat_map(|((seq, exec_digest), tx)| {
                            let digest = exec_digest.transaction;
                            let tx_data = tx.transaction.transaction_data();

                            let affected = std::iter::once(tx_data.sender())
                                .chain(std::iter::once(tx_data.gas_owner()))
                                .chain(tx.effects.all_changed_objects().into_iter().filter_map(
                                    |(_object_ref, owner, _write_kind)| match owner {
                                        Owner::Address(a) => Some(a),
                                        _ => None,
                                    },
                                ))
                                .collect::<HashSet<IotaAddress>>();

                            affected
                                .into_iter()
                                .map(move |address| (address, seq, digest))
                        });

                    client
                        .save_transactions_by_address(entries_by_address)
                        .await?;
                }
                Table::Checkpoints => {
                    client.save_checkpoint(&checkpoint).await?;
                }
                Table::CheckpointsByDigest => {
                    client.save_checkpoint_by_digest(&checkpoint).await?;
                }
            }
        }
        Ok(())
    }
}
