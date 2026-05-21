// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashSet, sync::Arc};

use async_trait::async_trait;
use iota_data_ingestion_core::Worker;
use iota_types::{
    base_types::IotaAddress, effects::TransactionEffectsExt,
    full_checkpoint_content::CheckpointData, object::Owner, transaction::TransactionDataAPI,
};
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

use crate::{BigTableClient, KeyValueStoreWriter, TransactionData};

/// Represents the BigTable tables used by the KvWorker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, strum::EnumIter)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum BigtableTable {
    Objects,
    Transactions,
    TransactionsByAddress,
    Checkpoints,
}

/// This worker implementation is responsible for processing checkpoints by
/// storing its data as Key-Value pairs. The Key-Value pairs are stored in a
/// BigTableDB.
pub struct KvWorker {
    pub client: BigTableClient,
    /// The tables enabled for writing by this worker.
    pub enabled_tables: Vec<BigtableTable>,
}

impl KvWorker {
    /// Creates a new KvWorker with the specified BigTable client and enabled
    /// tables the user wishes to write to.
    ///
    /// If `tables` is `None`, all available tables will be enabled.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use iota_kvstore::KvWorker;
    /// # use iota_kvstore::{BigtableTable, BigTableClient};
    /// # use std::collections::HashSet;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let client = BigTableClient::new_local("instance_id", "column_family")
    ///     .await
    ///     .unwrap();
    ///
    /// /// Write all available tables to BigTable.
    /// let worker = KvWorker::new(client.clone(), None);
    ///
    /// /// Write only the `Objects` table to BigTable.
    /// let worker = KvWorker::new(client.clone(), HashSet::from([BigtableTable::Objects]));
    ///
    /// # drop(worker);
    /// # }
    /// ```
    pub fn new(client: BigTableClient, tables: impl Into<Option<HashSet<BigtableTable>>>) -> Self {
        let tables = tables.into();
        // handle the case where the user provided an empty set of tables.
        let tables = tables.filter(|s| !s.is_empty());
        let enabled_tables = BigtableTable::iter()
            .filter(|t| tables.as_ref().is_none_or(|set| set.contains(t)))
            .collect();
        Self {
            client,
            enabled_tables,
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
                BigtableTable::Objects => {
                    let objects = checkpoint
                        .transactions
                        .iter()
                        .flat_map(|t| &t.output_objects)
                        .collect::<Vec<_>>();
                    client.save_objects(&objects).await?;
                }
                BigtableTable::Transactions => {
                    let transactions = checkpoint
                        .transactions
                        .iter()
                        .map(|t| {
                            TransactionData::new(t, checkpoint.checkpoint_summary.sequence_number)
                        })
                        .collect::<Vec<_>>();
                    client.save_transactions(&transactions).await?;
                }
                BigtableTable::TransactionsByAddress => {
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
                BigtableTable::Checkpoints => {
                    client.save_checkpoint(&checkpoint).await?;
                }
            }
        }
        Ok(())
    }
}
