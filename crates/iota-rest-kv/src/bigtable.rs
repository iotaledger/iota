// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This module provides a client for interacting with the key-value store.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use anyhow::Result;
use bytes::Bytes;
use iota_kvstore::{BigTableClient, KeyValueStoreReader};
use iota_storage::http_key_value_store::Key;
use iota_types::storage::ObjectKey;
use serde::{Deserialize, Serialize};

use crate::errors::ApiError;

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

    /// Gets value as [`Bytes`] from the kv store.
    ///
    /// Based on the provided [`Key`] fetch the data from BigTableDB.
    pub async fn get(&self, key: Key) -> Result<Option<Bytes>, ApiError> {
        let results = self.multi_get(vec![key]).await?;
        Ok(results.into_iter().next().unwrap_or(None))
    }

    /// Gets multiple values as [`Vec`]<[`Option`]<[`Bytes`]>> from the kv
    /// store.
    ///
    /// Based on the provided [`Vec`]<[`Key`]> fetch the data from BigTableDB.
    /// Returns a vector of the same length and order as the input keys.
    /// Each entry is `Some(bytes)` if the key was found, or `None` if not
    /// found.
    ///
    /// All keys must be of the same type, otherwise [`ApiError::BadRequest`] is
    /// returned.
    pub async fn multi_get(&self, keys: Vec<Key>) -> Result<Vec<Option<Bytes>>, ApiError> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        let mut client = self.bigtable_client.clone();

        // Use the first key to determine the type - all keys should be of the same type
        match keys.first().expect("emptiness was checked earlier") {
            Key::Transaction(_) => {
                let digests = Self::extract_keys(&keys, |k| match k {
                    Key::Transaction(digest) => Some(*digest),
                    _ => None,
                })?;

                let transactions = client.get_transactions(&digests).await?;

                let ordered = Self::restore_order(digests, transactions, |tx_data| {
                    *tx_data.transaction.digest()
                });
                Self::extract_values_and_serialize(ordered, |tx_data| Some(&tx_data.transaction))
            }
            Key::TransactionEffects(_) => {
                let digests = Self::extract_keys(&keys, |k| match k {
                    Key::TransactionEffects(digest) => Some(*digest),
                    _ => None,
                })?;

                let transactions = client.get_transactions(&digests).await?;

                let ordered = Self::restore_order(digests, transactions, |tx_data| {
                    *tx_data.transaction.digest()
                });
                Self::extract_values_and_serialize(ordered, |tx_data| Some(&tx_data.effects))
            }
            Key::CheckpointContents(_) => {
                let seq_nums = Self::extract_keys(&keys, |k| match k {
                    Key::CheckpointContents(seq_num) => Some(*seq_num),
                    _ => None,
                })?;

                let checkpoints = client.get_checkpoints(&seq_nums).await?;

                let ordered = Self::restore_order(seq_nums, checkpoints, |chk| {
                    *chk.summary.data().sequence_number()
                });
                Self::extract_values_and_serialize(ordered, |chk| Some(&chk.contents))
            }
            Key::CheckpointSummary(_) => {
                let seq_nums = Self::extract_keys(&keys, |k| match k {
                    Key::CheckpointSummary(seq_num) => Some(*seq_num),
                    _ => None,
                })?;

                let checkpoints = client.get_checkpoints(&seq_nums).await?;

                let ordered = Self::restore_order(seq_nums, checkpoints, |chk| {
                    *chk.summary.data().sequence_number()
                });
                Self::extract_values_and_serialize(ordered, |chk| Some(&chk.summary))
            }
            Key::CheckpointSummaryByDigest(_) => {
                let checkpoint_digests = Self::extract_keys(&keys, |k| match k {
                    Key::CheckpointSummaryByDigest(checkpoint_digest) => Some(*checkpoint_digest),
                    _ => None,
                })?;

                let checkpoints = client
                    .get_checkpoints_by_digest(&checkpoint_digests)
                    .await?;

                let ordered = Self::restore_order(checkpoint_digests, checkpoints, |chk| {
                    *chk.summary.digest()
                });
                Self::extract_values_and_serialize(ordered, |chk| Some(&chk.summary))
            }
            Key::TransactionToCheckpoint(_) => {
                let digests = Self::extract_keys(&keys, |k| match k {
                    Key::TransactionToCheckpoint(digest) => Some(*digest),
                    _ => None,
                })?;

                let transactions = client.get_transactions(&digests).await?;

                let ordered = Self::restore_order(digests, transactions, |tx_data| {
                    *tx_data.transaction.digest()
                });
                Self::extract_values_and_serialize(ordered, |tx_data| {
                    Some(&tx_data.checkpoint_number)
                })
            }
            Key::ObjectKey(_, _) => {
                let object_keys = Self::extract_keys(&keys, |k| match k {
                    Key::ObjectKey(object_id, sequence_number) => {
                        Some(ObjectKey(*object_id, *sequence_number))
                    }
                    _ => None,
                })?;

                let objects = client.get_objects(&object_keys).await?;

                let ordered = Self::restore_order(object_keys, objects, |object| {
                    ObjectKey(object.id(), object.version())
                });
                Self::extract_values_and_serialize(ordered, |object| Some(object))
            }
            Key::EventsByTransactionDigest(_) => {
                let digests = Self::extract_keys(&keys, |k| match k {
                    Key::EventsByTransactionDigest(digest) => Some(*digest),
                    _ => None,
                })?;

                let transactions = client.get_transactions(&digests).await?;

                let ordered = Self::restore_order(digests, transactions, |tx_data| {
                    *tx_data.transaction.digest()
                });
                Self::extract_values_and_serialize(ordered, |tx_data| tx_data.events.as_ref())
            }
        }
        .map_err(Into::into)
    }

    /// Extracts specific key type from a general [`Key`] type.
    ///
    /// Takes:
    /// - `keys`: The list of keys to extract from
    /// - `extractor`: Function that returns Some(extracted_value) for the
    ///   target variant, None otherwise
    ///
    /// Returns a vector of extracted values. Returns [`ApiError::BadRequest`]
    /// if any extraction returns None value.
    fn extract_keys<T, F>(keys: &[Key], extractor: F) -> Result<Vec<T>, ApiError>
    where
        F: Fn(&Key) -> Option<T>,
    {
        keys.iter()
            .map(|k| {
                extractor(k).ok_or_else(|| {
                    ApiError::BadRequest("all keys should be of the same type".to_string())
                })
            })
            .collect()
    }

    /// Maps `results` from KV back to original order specified by `keys`.
    ///
    /// Takes:
    /// - `keys`: The original list of keys in the order they were requested
    /// - `results`: The results returned from the KV client (may be in
    ///   different order or missing items)
    /// - `key_extractor`: Function to extract the key from a result item
    ///
    /// Returns a vector in the same order as `keys`, with `Some(T)` for found
    /// items and `None` for missing items.
    fn restore_order<K, T, F>(keys: Vec<K>, results: Vec<T>, key_extractor: F) -> Vec<Option<T>>
    where
        K: std::hash::Hash + Eq,
        T: Clone,
        F: Fn(&T) -> K,
    {
        // Create a map from key to result data
        let results_map: HashMap<K, T> = results
            .into_iter()
            .map(|item| (key_extractor(&item), item))
            .collect();

        // Map back to original order
        keys.into_iter()
            .map(|key| results_map.get(&key).cloned())
            .collect()
    }

    /// Extracts final values from KV response and serializes them.
    ///
    /// Takes:
    /// - `ordered_results`: Results in the desired order (with None for missing
    ///   items)
    /// - `value_extractor`: Function to extract the value from a result item
    ///
    /// Returns a vector of serialized bytes, with `None` for missing or empty
    /// values.
    fn extract_values_and_serialize<T, V, G>(
        ordered_results: Vec<Option<T>>,
        value_extractor: G,
    ) -> Result<Vec<Option<Bytes>>>
    where
        V: Serialize,
        G: Fn(&T) -> Option<&V>,
    {
        ordered_results
            .iter()
            .map(|opt_item| {
                opt_item
                    .as_ref()
                    .and_then(&value_extractor)
                    .map(|value| bcs::to_bytes(value).map(Bytes::from))
                    .transpose()
                    .map_err(Into::into)
            })
            .collect::<Result<Vec<_>>>()
    }
}
