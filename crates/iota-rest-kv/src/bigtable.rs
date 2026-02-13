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
use iota_types::{effects::TransactionEvents, storage::ObjectKey};
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
    #[serde(skip_serializing_if = "Option::is_none")]
    emulator_host: Option<String>,
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
        let bigtable_client = if let Some(emulator_host) = config.emulator_host {
            std::env::set_var("BIGTABLE_EMULATOR_HOST", &emulator_host);
            BigTableClient::new_local(config.instance_id, config.column_family).await?
        } else {
            BigTableClient::new_remote(
                config.instance_id,
                true,
                Some(Duration::from_secs(config.timeout_secs as u64)),
                "rest".to_string(),
                config.column_family,
                None,
            )
            .await?
        };

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

                let keys = digests.iter().map(|tx| Some(tx.inner().to_vec())).collect();

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

                let keys = digests.iter().map(|tx| Some(tx.inner().to_vec())).collect();

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
                    .map(|sq| Some(sq.to_be_bytes().to_vec()))
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
                    .map(|sq| Some(sq.to_be_bytes().to_vec()))
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

                let digest_keys = checkpoint_digests
                    .iter()
                    .map(|digest| Some(digest.inner().to_vec()))
                    .collect::<Vec<Option<Vec<u8>>>>();

                fetch_checkpoint_summary_by_digests(&mut client, digest_keys).await
            }
            Key::TransactionToCheckpoint(_) => {
                let digests = Self::extract_keys(&keys, |k| match k {
                    Key::TransactionToCheckpoint(digest) => Some(*digest),
                    _ => None,
                })?;

                let keys = digests.iter().map(|tx| Some(tx.inner().to_vec())).collect();

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

                let keys = object_keys
                    .iter()
                    .map(|key| Some(raw_object_key(key)))
                    .collect();

                multi_get_cell(&mut client, OBJECTS_TABLE, keys, DEFAULT_COLUMN_QUALIFIER).await
            }
            Key::EventsByTransactionDigest(_) => {
                let digests = Self::extract_keys(&keys, |k| match k {
                    Key::EventsByTransactionDigest(digest) => Some(*digest),
                    _ => None,
                })?;

                let keys = digests.iter().map(|tx| Some(tx.inner().to_vec())).collect();

                let response = multi_get_cell(
                    &mut client,
                    TRANSACTIONS_TABLE,
                    keys,
                    EVENTS_COLUMN_QUALIFIER,
                )
                .await?;

                Ok(response
                    .into_iter()
                    .map(|cell| {
                        cell.and_then(|bytes| {
                            match bcs::from_bytes::<Option<TransactionEvents>>(&bytes) {
                                Ok(None) | Err(_) => None,
                                Ok(Some(events)) => bcs::to_bytes(&events).map(Bytes::from).ok(),
                            }
                        })
                    })
                    .collect())
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
/// Keys wrapped in `Option<Vec<u8>>` allow chaining multiple queries: the
/// result from one `multi_get_cell` (which contains `None` for missing keys)
/// can be directly passed as input to the next call. `None` keys are skipped in
/// the query but preserve their position in the result.
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
    keys: Vec<Option<Vec<u8>>>,
    column_qualifier: &str,
) -> Result<Vec<Option<Bytes>>, anyhow::Error> {
    // pre-allocate results with None. Matching cells will replace None with
    // Some(value), and unmatched keys will remain None.
    let mut results = vec![None; keys.len()];

    let key_to_index = keys
        .iter()
        .enumerate()
        .filter_map(|(index, key)| key.as_ref().map(|k| (k.clone(), index)))
        .collect::<HashMap<Vec<u8>, usize>>();

    // create the exact match filter
    // We use ^ and $ to ensure it's an exact byte match, not a substring match.
    let exact_column_filter = RowFilter {
        filter: Some(Filter::ColumnQualifierRegexFilter(
            format!("^{column_qualifier}$").into_bytes(),
        )),
    };

    for row in client
        .multi_get(
            table_name,
            key_to_index.keys().cloned().collect(),
            Some(exact_column_filter),
        )
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

/// Fetch multiple checkpoint summaries by its checkpoint digest.
async fn fetch_checkpoint_summary_by_digests(
    client: &mut BigTableClient,
    keys: Vec<Option<Vec<u8>>>,
) -> Result<Vec<Option<Bytes>>, anyhow::Error> {
    let sequence_numbers = multi_get_cell(
        client,
        CHECKPOINTS_BY_DIGEST_TABLE,
        keys,
        DEFAULT_COLUMN_QUALIFIER,
    )
    .await?;

    let seq_numbers_keys = sequence_numbers
        .into_iter()
        .map(|bytes| bytes.map(|b| b.to_vec()))
        .collect::<Vec<Option<Vec<u8>>>>();

    multi_get_cell(
        client,
        CHECKPOINTS_TABLE,
        seq_numbers_keys,
        CHECKPOINT_SUMMARY_COLUMN_QUALIFIER,
    )
    .await
}
