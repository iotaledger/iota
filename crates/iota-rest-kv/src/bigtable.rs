// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This module provides a client for interacting with the key-value store.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use anyhow::Result;
use bytes::Bytes;
use futures::{StreamExt, TryStreamExt, stream};
use iota_kvstore::{
    BigTableClient, Cell,
    client::{
        CHECKPOINT_CONTENTS_COLUMN_QUALIFIER, CHECKPOINT_SUMMARY_COLUMN_QUALIFIER,
        CHECKPOINTS_BY_DIGEST_TABLE, CHECKPOINTS_TABLE, DEFAULT_COLUMN_QUALIFIER,
        EFFECTS_COLUMN_QUALIFIER, EVENTS_COLUMN_QUALIFIER, OBJECTS_TABLE,
        TRANSACTION_COLUMN_QUALIFIER, TRANSACTION_TO_CHECKPOINT, TRANSACTIONS_TABLE,
        raw_object_key,
    },
    proto::bigtable::v2::{
        RowFilter,
        row_filter::Filter,
        row_range::{EndKey, StartKey},
    },
};
use iota_storage::http_key_value_store::{ItemType, Key};
use iota_types::{effects::TransactionEvents, storage::ObjectKey};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::errors::{ApiError, RangeKeyBoundError};

/// The maximum number of concurrent futures allowed when scanning BigTableDB.
const MAX_CONCURRENT_FUTURES: usize = 100;

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

    /// Builds a [`RowFilter`] that matches only cells whose column qualifier
    /// equals `column_qualifier` exactly.
    ///
    /// Internally the filter is implemented as a regex anchored with `^` and
    /// `$` to enforce an exact byte match of the provided
    /// `column_qualifier`, preventing partial or substring matches against
    /// other columns whose qualifiers happen to contain `column_qualifier`
    /// as a prefix, suffix, or substring.
    fn column_qualifier_filter(column_qualifier: &str) -> RowFilter {
        RowFilter {
            filter: Some(Filter::ColumnQualifierRegexFilter(
                format!("^{column_qualifier}$").into_bytes(),
            )),
        }
    }

    /// Get the elapsed time from which the service was instantiated.
    pub fn get_uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Gets value as [`Bytes`] from the kv store.
    ///
    /// Based on the provided [`Key`] fetch the data from BigTableDB.
    pub async fn get(&self, key: Key) -> Result<Option<Bytes>, ApiError> {
        let results = self.get_items(vec![key]).await?;
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
    pub async fn get_items(&self, keys: Vec<Key>) -> Result<Vec<Option<Bytes>>, ApiError> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        // Use the first key to determine the type - all keys should be of the same type
        match keys.first().expect("emptiness was checked earlier") {
            Key::Transaction(_) => {
                let digests = extract_keys(&keys, |k| match k {
                    Key::Transaction(digest) => Some(*digest),
                    _ => None,
                })?;

                let keys = digests.iter().map(|tx| Some(tx.inner().to_vec())).collect();

                self.fetch_from_bigtable(TRANSACTIONS_TABLE, keys, TRANSACTION_COLUMN_QUALIFIER)
                    .await
            }
            Key::TransactionEffects(_) => {
                let digests = extract_keys(&keys, |k| match k {
                    Key::TransactionEffects(digest) => Some(*digest),
                    _ => None,
                })?;

                let keys = digests.iter().map(|tx| Some(tx.inner().to_vec())).collect();

                self.fetch_from_bigtable(TRANSACTIONS_TABLE, keys, EFFECTS_COLUMN_QUALIFIER)
                    .await
            }
            Key::CheckpointContents(_) => {
                let seq_nums = extract_keys(&keys, |k| match k {
                    Key::CheckpointContents(seq_num) => Some(*seq_num),
                    _ => None,
                })?;

                let keys = seq_nums
                    .iter()
                    .map(|sq| Some(sq.to_be_bytes().to_vec()))
                    .collect();

                self.fetch_from_bigtable(
                    CHECKPOINTS_TABLE,
                    keys,
                    CHECKPOINT_CONTENTS_COLUMN_QUALIFIER,
                )
                .await
            }
            Key::CheckpointSummary(_) => {
                let seq_nums = extract_keys(&keys, |k| match k {
                    Key::CheckpointSummary(seq_num) => Some(*seq_num),
                    _ => None,
                })?;

                let keys = seq_nums
                    .iter()
                    .map(|sq| Some(sq.to_be_bytes().to_vec()))
                    .collect();

                self.fetch_from_bigtable(
                    CHECKPOINTS_TABLE,
                    keys,
                    CHECKPOINT_SUMMARY_COLUMN_QUALIFIER,
                )
                .await
            }
            Key::CheckpointSummaryByDigest(_) => {
                let checkpoint_digests = extract_keys(&keys, |k| match k {
                    Key::CheckpointSummaryByDigest(checkpoint_digest) => Some(*checkpoint_digest),
                    _ => None,
                })?;

                let digest_keys = checkpoint_digests
                    .iter()
                    .map(|digest| Some(digest.inner().to_vec()))
                    .collect::<Vec<Option<Vec<u8>>>>();

                self.checkpoint_summary_by_digests(digest_keys).await
            }
            Key::TransactionToCheckpoint(_) => {
                let digests = extract_keys(&keys, |k| match k {
                    Key::TransactionToCheckpoint(digest) => Some(*digest),
                    _ => None,
                })?;

                let keys = digests.iter().map(|tx| Some(tx.inner().to_vec())).collect();

                self.fetch_from_bigtable(TRANSACTIONS_TABLE, keys, TRANSACTION_TO_CHECKPOINT)
                    .await
            }
            Key::ObjectKey(_) => {
                let object_keys = extract_keys(&keys, |k| match k {
                    Key::ObjectKey(object_key) => Some(*object_key),
                    _ => None,
                })?;

                let keys = object_keys
                    .iter()
                    .map(|key| Some(raw_object_key(key)))
                    .collect();

                self.fetch_from_bigtable(OBJECTS_TABLE, keys, DEFAULT_COLUMN_QUALIFIER)
                    .await
            }
            Key::EventsByTransactionDigest(_) => {
                let digests = extract_keys(&keys, |k| match k {
                    Key::EventsByTransactionDigest(digest) => Some(*digest),
                    _ => None,
                })?;

                let keys = digests.iter().map(|tx| Some(tx.inner().to_vec())).collect();

                let response = self
                    .fetch_from_bigtable(TRANSACTIONS_TABLE, keys, EVENTS_COLUMN_QUALIFIER)
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

    /// Fetch multiple values from a BigTable table with a specific key and
    /// column qualifier.
    ///
    /// Keys wrapped in `Option<Vec<u8>>` allow chaining multiple queries: the
    /// result from one `fetch_from_bigtable` (which contains `None` for missing
    /// keys) can be directly passed as input to the next call. `None` keys
    /// are skipped in the query but preserve their position in the result.
    ///
    /// The result's length is guaranteed to match the input `keys` length. Each
    /// position in the result corresponds to the key at the same position in
    /// the input. This allows the caller to easily determine which
    /// requested keys have data:
    /// - `Some(value)` at index `i` means `key[i]` exists and has data
    /// - `None` at index `i` means `key[i]` was not found or has no matching
    ///   data
    async fn fetch_from_bigtable(
        &self,
        table_name: &str,
        keys: Vec<Option<Vec<u8>>>,
        column_qualifier: &str,
    ) -> Result<Vec<Option<Bytes>>, anyhow::Error> {
        let mut client = self.bigtable_client.clone();
        // pre-allocate results with None. Matching cells will replace None with
        // Some(value), and unmatched keys will remain None.
        let mut results = vec![None; keys.len()];

        let key_to_index = keys
            .iter()
            .enumerate()
            .filter_map(|(index, key)| key.as_ref().map(|k| (k.clone(), index)))
            .collect::<HashMap<Vec<u8>, usize>>();

        for row in client
            .multi_get(
                table_name,
                key_to_index.keys().cloned().collect(),
                Some(Self::column_qualifier_filter(column_qualifier)),
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
                    error!("unexpected column {cell_name:?} in {table_name} table")
                }
            }
        }

        Ok(results)
    }

    /// Fetch multiple checkpoint summaries by its checkpoint digest.
    async fn checkpoint_summary_by_digests(
        &self,
        keys: Vec<Option<Vec<u8>>>,
    ) -> Result<Vec<Option<Bytes>>, anyhow::Error> {
        let sequence_numbers = self
            .fetch_from_bigtable(CHECKPOINTS_BY_DIGEST_TABLE, keys, DEFAULT_COLUMN_QUALIFIER)
            .await?;

        let seq_numbers_keys = sequence_numbers
            .into_iter()
            .map(|bytes| bytes.map(|b| b.to_vec()))
            .collect::<Vec<Option<Vec<u8>>>>();

        self.fetch_from_bigtable(
            CHECKPOINTS_TABLE,
            seq_numbers_keys,
            CHECKPOINT_SUMMARY_COLUMN_QUALIFIER,
        )
        .await
    }

    /// Performs a single reverse range scan against the objects table and
    /// returns the bytes of the highest stored version that is **strictly
    /// less than** the version encoded in [`ObjectRangeKeyBound::end_key`].
    ///
    /// Returns `None` if no version below the requested one exists for that
    /// object (or if the object is not stored at all).
    pub async fn object_before_version(
        &self,
        range: ObjectRangeKeyBound,
    ) -> Result<Option<Bytes>, anyhow::Error> {
        if range.is_empty() {
            return Ok(None);
        }

        let mut client = self.bigtable_client.clone();

        let reversed = true;
        let rows_limit = 1;
        let rows = client
            .range_scan(
                OBJECTS_TABLE,
                Some(range.start_key),
                Some(range.end_key),
                rows_limit,
                reversed,
                Some(Self::column_qualifier_filter(DEFAULT_COLUMN_QUALIFIER)),
            )
            .await?;

        let Some(row) = rows.into_iter().next() else {
            return Ok(None);
        };

        let mut value = None;
        for Cell {
            name,
            value: cell_value,
        } in row.cells
        {
            // little optimization, comparing bytes is cheaper than converting it to utf8
            // string and do string comparison.
            if name == DEFAULT_COLUMN_QUALIFIER.as_bytes() {
                value = Some(Bytes::from(cell_value));
            } else {
                let cell_name = std::str::from_utf8(&name)?;
                error!("unexpected column {cell_name:?} in {OBJECTS_TABLE} table");
            }
        }
        Ok(value)
    }

    /// Gets multiple objects returning the latest version strictly less than
    /// the requested version values as [`Vec`]<[`Option`]<[`Bytes`]>> from the
    /// kv store.
    ///
    /// Based on the provided [`ObjectsBeforeVersionRequest`] fetch the data
    /// from BigTableDB. Returns a vector of the same length and order as
    /// the input keys. Each entry is `Some(bytes)` if the key was found, or
    /// `None` if not found.
    pub async fn objects_before_version(
        &self,
        req: ObjectsBeforeVersionRequest,
    ) -> Result<Vec<Option<Bytes>>, anyhow::Error> {
        let req = req.into_inner();

        // `multiget_max_items` bounds the size of an incoming request, but it is
        // an operator-configurable value. Concurrency is capped independently via
        // `MAX_CONCURRENT_FUTURES` so that increasing the request limit does not
        // proportionally increase the number of concurrent BigTable calls per request.
        let max_buffered_concurrent_futures = req.len().clamp(1, MAX_CONCURRENT_FUTURES);

        stream::iter(req)
            .map(|range| self.object_before_version(range))
            .buffered(max_buffered_concurrent_futures)
            .try_collect::<Vec<Option<Bytes>>>()
            .await
    }
}

/// Extracts specific key type from a general [`Key`] type.
///
/// Takes:
/// - `keys`: The list of keys to extract from
/// - `extractor`: Function that returns Some(extracted_value) for the target
///   variant, None otherwise
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

/// Represents a range of keys for a single object the BigTableDB is allowed to
/// scan.
pub(crate) struct ObjectRangeKeyBound {
    start_key: StartKey,
    end_key: EndKey,
}

impl ObjectRangeKeyBound {
    /// Returns `true` if the range contains no rows.
    fn is_empty(&self) -> bool {
        match (&self.start_key, &self.end_key) {
            (StartKey::StartKeyClosed(start), EndKey::EndKeyClosed(end)) => start > end,
            (
                StartKey::StartKeyClosed(start) | StartKey::StartKeyOpen(start),
                EndKey::EndKeyClosed(end) | EndKey::EndKeyOpen(end),
            ) => start >= end,
        }
    }
}

impl TryFrom<Key> for ObjectRangeKeyBound {
    type Error = RangeKeyBoundError;

    fn try_from(key: Key) -> Result<Self, Self::Error> {
        let Key::ObjectKey(object_key) = key else {
            return Err(RangeKeyBoundError::UnexpectedItemType {
                expected: ItemType::Object,
                detail: "range key must be an ObjectKey".into(),
            });
        };
        Ok(object_key.into())
    }
}

impl From<ObjectKey> for ObjectRangeKeyBound {
    fn from(value: ObjectKey) -> Self {
        let obj_id = value.0;
        ObjectRangeKeyBound {
            start_key: StartKey::StartKeyClosed(raw_object_key(&ObjectKey::min_for_id(&obj_id))),
            end_key: EndKey::EndKeyOpen(raw_object_key(&value)),
        }
    }
}

/// Represents a request to fetch objects before a given version.
///
/// This struct expects that all keys in the input vector are of type
/// [`Key::ObjectKey`] in the [`TryFrom`] implementation.
pub(crate) struct ObjectsBeforeVersionRequest(Vec<ObjectRangeKeyBound>);

impl ObjectsBeforeVersionRequest {
    fn into_inner(self) -> Vec<ObjectRangeKeyBound> {
        self.0
    }
}

impl TryFrom<Vec<Key>> for ObjectsBeforeVersionRequest {
    type Error = RangeKeyBoundError;

    fn try_from(keys: Vec<Key>) -> Result<Self, Self::Error> {
        let object_range_keys = extract_keys(&keys, |k| match k {
            Key::ObjectKey(object_key) => Some(*object_key),
            _ => None,
        })
        .map_err(|e| RangeKeyBoundError::UnexpectedItemType {
            expected: ItemType::Object,
            detail: e.to_string(),
        })?
        .into_iter()
        .map(ObjectRangeKeyBound::from)
        .collect();

        Ok(ObjectsBeforeVersionRequest(object_range_keys))
    }
}
