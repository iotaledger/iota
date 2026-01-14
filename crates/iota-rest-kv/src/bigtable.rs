// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This module provides a client for interacting with the key-value store.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use anyhow::Result;
use bytes::Bytes;
use iota_kvstore::{
    BigTableClient, Cell,
    client::{
        CHECKPOINT_CONTENTS_COLUMN_QUALIFIER, CHECKPOINT_SUMMARY_COLUMN_QUALIFIER,
        CHECKPOINTS_BY_DIGEST_TABLE, CHECKPOINTS_TABLE, DEFAULT_COLUMN_QUALIFIER,
        EFFECTS_COLUMN_QUALIFIER, EVENTS_COLUMN_QUALIFIER, OBJECTS_TABLE,
        TRANSACTION_COLUMN_QUALIFIER, TRANSACTION_TO_CHECKPOINT, TRANSACTIONS_TABLE,
        raw_object_key,
    },
    proto::bigtable::v2::{RowFilter, row_filter::Filter},
};
use iota_storage::http_key_value_store::Key;
use iota_types::storage::ObjectKey;
use serde::{Deserialize, Serialize};
use tracing::error;

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

                let keys = digests.iter().map(|tx| tx.inner().to_vec()).collect();

                multi_get_cell(
                    &mut client,
                    TRANSACTIONS_TABLE,
                    keys,
                    TRANSACTION_COLUMN_QUALIFIER,
                )
                .await
            }
            Key::TransactionEffects(_) => {
                let digests = Self::extract_keys(&keys, |k| match k {
                    Key::TransactionEffects(digest) => Some(*digest),
                    _ => None,
                })?;

                let keys = digests.iter().map(|tx| tx.inner().to_vec()).collect();

                multi_get_cell(
                    &mut client,
                    TRANSACTIONS_TABLE,
                    keys,
                    EFFECTS_COLUMN_QUALIFIER,
                )
                .await
            }
            Key::CheckpointContents(_) => {
                let seq_nums = Self::extract_keys(&keys, |k| match k {
                    Key::CheckpointContents(seq_num) => Some(*seq_num),
                    _ => None,
                })?;

                let keys = seq_nums
                    .iter()
                    .map(|sq| sq.to_be_bytes().to_vec())
                    .collect();

                multi_get_cell(
                    &mut client,
                    CHECKPOINTS_TABLE,
                    keys,
                    CHECKPOINT_CONTENTS_COLUMN_QUALIFIER,
                )
                .await
            }
            Key::CheckpointSummary(_) => {
                let seq_nums = Self::extract_keys(&keys, |k| match k {
                    Key::CheckpointSummary(seq_num) => Some(*seq_num),
                    _ => None,
                })?;

                let keys = seq_nums
                    .iter()
                    .map(|sq| sq.to_be_bytes().to_vec())
                    .collect();

                multi_get_cell(
                    &mut client,
                    CHECKPOINTS_TABLE,
                    keys,
                    CHECKPOINT_SUMMARY_COLUMN_QUALIFIER,
                )
                .await
            }
            Key::CheckpointSummaryByDigest(_) => {
                let checkpoint_digests = Self::extract_keys(&keys, |k| match k {
                    Key::CheckpointSummaryByDigest(checkpoint_digest) => Some(*checkpoint_digest),
                    _ => None,
                })?;

                // pre-allocate results with None. Matching cells will replace None with
                // Some(value), and unmatched keys will remain None.
                let mut results = vec![None; keys.len()];

                let digest_keys = checkpoint_digests
                    .iter()
                    .map(|digest| digest.inner().to_vec())
                    .collect::<Vec<Vec<u8>>>();

                // map digest key bytes to original index for later lookup.
                let digest_key_to_index = digest_keys
                    .iter()
                    .enumerate()
                    .map(|(index, key)| (key.clone(), index))
                    .collect::<HashMap<Vec<u8>, usize>>();

                // get checkpoint sequence numbers from provided digest keys.
                let digest_to_seq_num = client
                    .multi_get(CHECKPOINTS_BY_DIGEST_TABLE, digest_keys.clone(), None)
                    .await
                    .map_err(anyhow::Error::from)?
                    .into_iter()
                    .filter_map(|row| {
                        row.cells
                            .into_iter()
                            .next()
                            .map(|cell| (row.key, cell.value))
                    })
                    .collect::<HashMap<Vec<u8>, Vec<u8>>>();

                // map checkpoint sequence numbers to their digest indices to maintain order
                // through second query.
                let seq_num_to_digest_index = digest_to_seq_num
                    .iter()
                    .filter_map(|(digest_key, seq_num)| {
                        digest_key_to_index
                            .get(digest_key)
                            .map(|&index| (seq_num.clone(), index))
                    })
                    .collect::<HashMap<Vec<u8>, usize>>();

                // get checkpoint summaries using sequence numbers as keys.
                let seq_nums = digest_to_seq_num
                    .values()
                    .cloned()
                    .collect::<Vec<Vec<u8>>>();

                // narrow the search to only the checkpoint summaries.
                let exact_column_filter = RowFilter {
                    filter: Some(Filter::ColumnQualifierRegexFilter(
                        format!("^{CHECKPOINT_SUMMARY_COLUMN_QUALIFIER}$").into_bytes(),
                    )),
                };

                for row in client
                    .multi_get(CHECKPOINTS_TABLE, seq_nums, Some(exact_column_filter))
                    .await
                    .map_err(anyhow::Error::from)?
                {
                    for Cell { name, value } in row.cells {
                        let cell_name = std::str::from_utf8(&name).map_err(anyhow::Error::from)?;
                        if cell_name == CHECKPOINT_SUMMARY_COLUMN_QUALIFIER {
                            // map from sequence number back to original digest index
                            if let Some(&digest_index) = seq_num_to_digest_index.get(&row.key) {
                                results[digest_index] = Some(Bytes::from(value));
                            }
                        } else {
                            error!("unexpected column {cell_name:?} in checkpoints table")
                        }
                    }
                }

                Ok(results)
            }
            Key::TransactionToCheckpoint(_) => {
                let digests = Self::extract_keys(&keys, |k| match k {
                    Key::TransactionToCheckpoint(digest) => Some(*digest),
                    _ => None,
                })?;

                let keys = digests.iter().map(|tx| tx.inner().to_vec()).collect();

                multi_get_cell(
                    &mut client,
                    TRANSACTIONS_TABLE,
                    keys,
                    TRANSACTION_TO_CHECKPOINT,
                )
                .await
            }
            Key::ObjectKey(_, _) => {
                let object_keys = Self::extract_keys(&keys, |k| match k {
                    Key::ObjectKey(object_id, sequence_number) => {
                        Some(ObjectKey(*object_id, *sequence_number))
                    }
                    _ => None,
                })?;

                let keys = object_keys.iter().map(raw_object_key).collect();

                multi_get_cell(&mut client, OBJECTS_TABLE, keys, DEFAULT_COLUMN_QUALIFIER).await
            }
            Key::EventsByTransactionDigest(_) => {
                let digests = Self::extract_keys(&keys, |k| match k {
                    Key::EventsByTransactionDigest(digest) => Some(*digest),
                    _ => None,
                })?;

                let keys = digests.iter().map(|tx| tx.inner().to_vec()).collect();

                multi_get_cell(
                    &mut client,
                    TRANSACTIONS_TABLE,
                    keys,
                    EVENTS_COLUMN_QUALIFIER,
                )
                .await
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
}

/// Fetch multiple values from a BigTable table with a specific key and column
/// qualifier.
///
/// The result's length is guaranteed to match the input `keys` length. Each
/// position in the result corresponds to the key at the same position in the
/// input. This allows the caller to easily determine which requested keys have
/// data:
/// - `Some(value)` at index `i` means `key[i]` exists and has data
/// - `None` at index `i` means `key[i]` was not found or has no matching data
async fn multi_get_cell(
    client: &mut BigTableClient,
    table_name: &str,
    keys: Vec<Vec<u8>>,
    column_qualifier: &str,
) -> Result<Vec<Option<Bytes>>, anyhow::Error> {
    // pre-allocate results with None. Matching cells will replace None with
    // Some(value), and unmatched keys will remain None.
    let mut results = vec![None; keys.len()];

    let key_to_index: HashMap<Vec<u8>, usize> = keys
        .iter()
        .enumerate()
        .map(|(index, key)| (key.clone(), index))
        .collect();

    // create the exact match filter
    // We use ^ and $ to ensure it's an exact byte match, not a substring match.
    let exact_column_filter = RowFilter {
        filter: Some(Filter::ColumnQualifierRegexFilter(
            format!("^{column_qualifier}$").into_bytes(),
        )),
    };

    for row in client
        .multi_get(table_name, keys, Some(exact_column_filter))
        .await?
    {
        for Cell { name, value } in row.cells {
            let cell_name = std::str::from_utf8(&name)?;
            if cell_name == column_qualifier {
                if let Some(&index) = key_to_index.get(&row.key) {
                    results[index] = Some(Bytes::from(value));
                }
            } else {
                error!("unexpected column {cell_name:?} in checkpoints table")
            }
        }
    }

    Ok(results)
}
