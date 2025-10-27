// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This module provides a client for interacting with the key-value store.

use std::time::{Duration, Instant};

use anyhow::Result;
use bytes::Bytes;
use iota_kvstore::{BigTableClient, KeyValueStoreReader};
use iota_storage::http_key_value_store::Key;
use iota_types::storage::ObjectKey;
use serde::{Deserialize, Serialize};

/// Configuration for the [`KvStoreClient`] used to access data from BigTableDB
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct KvStoreConfig {
    instance_id: String,
    column_family: String,
    timeout_secs: usize,
}

/// Provides read access to data ingested by the `iota-data-ingestion`
/// crate's `KVStoreWorker`.
///
/// It retrieves data from BigTableDB.
///
/// The client implements a read-only interface and supports the HTTP fallback
/// mechanism used by
/// [`HttpKVStore`](iota_storage::http_key_value_store::HttpKVStore).
#[derive(Clone)]
pub struct KvStoreClient {
    /// BigTableDB client.
    bigtable_client: BigTableClient,
    /// The representation of the uptime of the service.
    start_time: Instant,
}

impl KvStoreClient {
    /// Create a new instance of the client.
    ///
    /// Internally it instantiates a BigTableDB client.
    pub async fn new(config: KvStoreConfig) -> Result<Self> {
        let bigtable_client = BigTableClient::new_remote(
            config.instance_id,
            true,
            Some(Duration::from_secs(config.timeout_secs as u64)),
            "rest".to_string(),
            config.column_family,
            None,
        )
        .await?;

        Ok(Self {
            bigtable_client,
            start_time: Instant::now(),
        })
    }

    /// Get the elapsed time from which the service was instantiated.
    pub fn get_uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Get value as [`Bytes`] from the kv store.
    ///
    /// Based on the provided [`Key`] fetch the data from BigTableDB.
    pub async fn get(&self, key: Key) -> Result<Option<Bytes>> {
        let mut client = self.bigtable_client.clone();
        match key {
            Key::Transaction(transaction_digest) => {
                let transactions = client.get_transactions(&[transaction_digest]).await?;

                let data = transactions
                    .first()
                    .map(|tx_data| bcs::to_bytes(&tx_data.transaction).map(Bytes::from))
                    .transpose()?;

                Ok(data)
            }
            Key::TransactionEffects(transaction_digest) => {
                let transactions = client.get_transactions(&[transaction_digest]).await?;

                let data = transactions
                    .first()
                    .map(|tx_data| bcs::to_bytes(&tx_data.effects).map(Bytes::from))
                    .transpose()?;

                Ok(data)
            }
            Key::CheckpointContents(chk_seq_num) => {
                let checkpoints = client.get_checkpoints(&[chk_seq_num]).await?;

                let data = checkpoints
                    .first()
                    .map(|chk| bcs::to_bytes(&chk.contents).map(Bytes::from))
                    .transpose()?;

                Ok(data)
            }
            Key::CheckpointSummary(chk_seq_num) => {
                let checkpoints = client.get_checkpoints(&[chk_seq_num]).await?;

                let data = checkpoints
                    .first()
                    .map(|chk| bcs::to_bytes(&chk.summary).map(Bytes::from))
                    .transpose()?;

                Ok(data)
            }
            Key::CheckpointSummaryByDigest(checkpoint_digest) => {
                let checkpoint = client.get_checkpoint_by_digest(checkpoint_digest).await?;

                let checkpoint = checkpoint
                    .map(|chk| bcs::to_bytes(&chk.summary).map(Bytes::from))
                    .transpose()?;

                Ok(checkpoint)
            }
            Key::TransactionToCheckpoint(transaction_digest) => {
                let transactions = client.get_transactions(&[transaction_digest]).await?;

                let data = transactions
                    .first()
                    .map(|tx_data| bcs::to_bytes(&tx_data.checkpoint_number).map(Bytes::from))
                    .transpose()?;

                Ok(data)
            }
            Key::ObjectKey(object_id, sequence_number) => {
                let object_key = ObjectKey(object_id, sequence_number);
                let objects = client.get_objects(&[object_key]).await?;

                let data = objects
                    .first()
                    .map(|object| bcs::to_bytes(object).map(Bytes::from))
                    .transpose()?;

                Ok(data)
            }
            Key::EventsByTransactionDigest(transaction_digest) => {
                let transactions = client.get_transactions(&[transaction_digest]).await?;

                let data = transactions
                    .first()
                    .and_then(|tx_data| {
                        tx_data
                            .events
                            .as_ref()
                            .map(|ev| bcs::to_bytes(ev).map(Bytes::from))
                    })
                    .transpose()?;

                Ok(data)
            }
        }
    }
}
