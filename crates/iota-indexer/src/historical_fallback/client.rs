// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Module containing the client for interacting with the REST API KV server.

use std::time::Duration;

use bytes::Bytes;
use futures::stream::{self, StreamExt};
use iota_storage::http_key_value_store::{ItemType, Key};
use iota_types::{
    base_types::{ObjectID, SequenceNumber},
    digests::{CheckpointDigest, TransactionDigest},
    effects::{TransactionEffects, TransactionEffectsAPI, TransactionEvents},
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContents, CheckpointSequenceNumber,
    },
    object::Object,
    transaction::Transaction,
};
use moka::sync::{Cache as MokaCache, CacheBuilder as MokaCacheBuilder};
use reqwest::{
    Client, Url,
    header::{CONTENT_LENGTH, HeaderValue},
};
use serde::{Deserialize, Serialize};
use tap::TapFallible;
use tracing::{error, info, instrument, trace, warn};

use crate::{
    IndexerError, errors::IndexerResult,
    historical_fallback::metrics::HistoricalFallbackClientMetrics,
};

const CACHE_TIME_TO_IDLE: Duration = Duration::from_secs(600);
/// Request payload for multi_get containing list of keys.
#[derive(Serialize, Debug)]
struct MultiGetRequest {
    /// The item type for all keys in this request.
    /// Not serialized - used only for URL construction.
    #[serde(skip)]
    item_type: ItemType,
    /// List of base64url-encoded keys to retrieve.
    keys: Vec<String>,
}

impl TryFrom<&[Key]> for MultiGetRequest {
    type Error = IndexerError;

    /// Creates a new MultiGetRequest from a slice of keys.
    /// All keys must be the same enum variant.
    ///
    /// # Errors
    /// Returns an error if keys are empty or if keys are different enum
    /// variants.
    fn try_from(keys: &[Key]) -> Result<Self, Self::Error> {
        if keys.is_empty() {
            return Err(IndexerError::InvalidArgument(
                "Cannot create MultiGetRequest with empty keys".to_string(),
            ));
        }

        let expected_discriminant = std::mem::discriminant(&keys[0]);
        let (item_type, _) = keys[0].to_path_elements();

        let mut encoded_keys = Vec::with_capacity(keys.len());
        for key in keys {
            if std::mem::discriminant(key) != expected_discriminant {
                return Err(IndexerError::NotSupported(
                    "MultiGetRequest with heterogenous Key variants are not supported.".to_string(),
                ));
            }
            let (_, encoded_key) = key.to_path_elements();
            encoded_keys.push(encoded_key);
        }

        Ok(Self {
            item_type,
            keys: encoded_keys,
        })
    }
}

pub(crate) trait KeyValueStoreClient {
    async fn multi_get_transactions(
        &self,
        transaction_digests: &[TransactionDigest],
    ) -> IndexerResult<Vec<Option<Transaction>>>;

    async fn multi_get_effects(
        &self,
        transaction_digests: &[TransactionDigest],
    ) -> IndexerResult<Vec<Option<TransactionEffects>>>;

    async fn multi_get_transactions_perpetual_checkpoints(
        &self,
        transaction_digests: &[TransactionDigest],
    ) -> IndexerResult<Vec<Option<CheckpointSequenceNumber>>>;

    async fn multi_get_events_by_tx_digests(
        &self,
        transaction_digests: &[TransactionDigest],
    ) -> IndexerResult<Vec<Option<TransactionEvents>>>;

    async fn multi_get_checkpoints_summaries_by_sequence_numbers(
        &self,
        checkpoint_sequence_numbers: &[CheckpointSequenceNumber],
    ) -> IndexerResult<Vec<Option<CertifiedCheckpointSummary>>>;

    async fn multi_get_checkpoints_contents(
        &self,
        checkpoint_sequence_numbers: &[CheckpointSequenceNumber],
    ) -> IndexerResult<Vec<Option<CheckpointContents>>>;

    async fn multi_get_checkpoints_summaries_by_digests(
        &self,
        checkpoint_digests: &[CheckpointDigest],
    ) -> IndexerResult<Vec<Option<CertifiedCheckpointSummary>>>;

    async fn multi_get_objects(
        &self,
        object_refs: &[(ObjectID, SequenceNumber)],
    ) -> IndexerResult<Vec<Option<Object>>>;
}

#[derive(Clone)]
pub(crate) struct HttpRestKVClient {
    base_url: Url,
    client: Client,
    /// Maximum number of keys per batch request
    batch_size: usize,
    /// Maximum number of concurrent batch requests
    max_concurrent_batches: usize,
    cache: MokaCache<Key, Bytes>,
    metrics: HistoricalFallbackClientMetrics,
}

impl HttpRestKVClient {
    pub fn new(
        base_url: &str,
        cache_size: u64,
        batch_size: usize,
        max_concurrent_batches: usize,
        metrics: HistoricalFallbackClientMetrics,
    ) -> IndexerResult<Self> {
        info!(
            "creating HttpRestKVClient with base_url: {base_url}, batch_size: {batch_size}, max_concurrent_batches: {max_concurrent_batches}",
        );

        let client = Client::builder().http2_prior_knowledge().build()?;

        let base_url = if base_url.ends_with('/') {
            base_url.to_string()
        } else {
            format!("{base_url}/")
        };

        let base_url = Url::parse(&base_url)?;

        let cache = MokaCacheBuilder::new(cache_size)
            .time_to_idle(CACHE_TIME_TO_IDLE)
            .build();

        Ok(Self {
            base_url,
            client,
            batch_size,
            max_concurrent_batches,
            metrics,
            cache,
        })
    }

    async fn multi_fetch(&self, keys: Vec<Key>) -> IndexerResult<Vec<Option<Bytes>>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        // pre-allocate results with None. Cache hits will replace None with
        // Some(value), and cache misses will remain None until we fetch them
        // from the REST API.
        let mut results = vec![None; keys.len()];
        // track keys that missed the cache, preserving their original positions.
        // Each entry is a tuple of (key, original_index) so we can merge fetched data
        // back into the correct position in the results vector.
        let mut missing = Vec::with_capacity(keys.len());

        for (index, key) in keys.iter().enumerate() {
            if let Some(bytes) = self.cache.get(key) {
                trace!("found cached data for url: {key:?}, len: {}", bytes.len());
                self.metrics.record_cache_hit(key.item_type());
                results[index] = Some(bytes);
            } else {
                self.metrics.record_cache_miss(key.item_type());
                missing.push((*key, index));
            }
        }

        if missing.is_empty() {
            return Ok(results);
        }

        let missing_chunks = missing
            .chunks(self.batch_size)
            .map(|chunk| {
                let keys = chunk.iter().map(|(key, _)| *key).collect::<Vec<Key>>();
                MultiGetRequest::try_from(keys.as_slice())
            })
            .collect::<Result<Vec<MultiGetRequest>, IndexerError>>()?;

        let mut fetch_batch_stream = stream::iter(missing_chunks)
            .map(|chunk| async move { self.fetch_batch(chunk).await })
            .buffered(self.max_concurrent_batches);

        let mut fetched_results = Vec::with_capacity(missing.len());
        while let Some(batch_result) = fetch_batch_stream.next().await {
            fetched_results.extend(batch_result?);
        }

        // process fetched results: for each missing key that was successfully fetched
        // that has non empty bytes, update the cache with the new data and
        // populate the corresponding slot in results at original index
        // position.
        for (fetch_result, (key, index)) in fetched_results.into_iter().zip(missing.into_iter()) {
            if let Some(bytes) = fetch_result.filter(|b| !b.is_empty()) {
                self.cache.insert(key, bytes.clone());
                results[index] = Some(bytes);
            }
        }

        Ok(results)
    }

    async fn fetch_batch(&self, request: MultiGetRequest) -> IndexerResult<Vec<Option<Bytes>>> {
        let url = self.base_url.join(&request.item_type.to_string())?;

        trace!(
            "fetching batch of {} keys from url: {url}",
            request.keys.len()
        );

        let resp = self.client.post(url.clone()).json(&request).send().await?;

        trace!(
            "got response {} for url: {url}, len: {:?}",
            resp.status(),
            resp.headers()
                .get(CONTENT_LENGTH)
                .unwrap_or(&HeaderValue::from_static("0"))
        );

        if !resp.status().is_success() {
            return Err(IndexerError::HistoricalFallbackStorageError(format!(
                "multi_fetch request failed with status: {}",
                resp.status()
            )));
        }

        let bytes = resp.bytes().await?;
        bcs::from_bytes::<Vec<Option<Bytes>>>(&bytes).map_err(|e| {
            IndexerError::Serde(format!("failed to deserialize multi_get response: {e:?}"))
        })
    }
}

fn deser<K, T>(key: &K, bytes: &[u8]) -> Option<T>
where
    K: std::fmt::Debug,
    T: for<'de> Deserialize<'de>,
{
    bcs::from_bytes(bytes)
        .tap_err(|e| {
            warn!(
                "Error deserializing data for key {key:?} into type {}: {e:?}",
                std::any::type_name::<T>()
            )
        })
        .ok()
}

fn deser_check_digest<T, D>(
    digest: &D,
    bytes: &Bytes,
    get_expected_digest: impl FnOnce(&T) -> D,
) -> Option<T>
where
    D: std::fmt::Debug + PartialEq,
    T: for<'de> Deserialize<'de>,
{
    deser(digest, bytes).and_then(|o: T| {
        let expected_digest = get_expected_digest(&o);
        if expected_digest == *digest {
            Some(o)
        } else {
            error!("Digest mismatch - expected: {digest:?}, got: {expected_digest:?}");
            None
        }
    })
}

impl KeyValueStoreClient for HttpRestKVClient {
    #[instrument(level = "trace", skip_all)]
    async fn multi_get_transactions(
        &self,
        transaction_digests: &[TransactionDigest],
    ) -> IndexerResult<Vec<Option<Transaction>>> {
        let keys = transaction_digests
            .iter()
            .map(|tx| Key::Transaction(*tx))
            .collect::<Vec<_>>();

        let fetches = self.multi_fetch(keys).await?;
        let txn_results = fetches
            .iter()
            .zip(transaction_digests.iter())
            .map(|(fetch, digest)| {
                fetch.as_ref().and_then(|bytes| {
                    deser_check_digest(digest, bytes, |tx: &Transaction| *tx.digest())
                })
            })
            .collect::<Vec<_>>();

        Ok(txn_results)
    }

    #[instrument(level = "trace", skip_all)]
    async fn multi_get_effects(
        &self,
        transaction_digests: &[TransactionDigest],
    ) -> IndexerResult<Vec<Option<TransactionEffects>>> {
        let keys = transaction_digests
            .iter()
            .map(|fx| Key::TransactionEffects(*fx))
            .collect::<Vec<_>>();

        let fetches = self.multi_fetch(keys).await?;
        let fx_results = fetches
            .iter()
            .zip(transaction_digests.iter())
            .map(|(fetch, digest)| {
                fetch.as_ref().and_then(|bytes| {
                    deser_check_digest(digest, bytes, |fx: &TransactionEffects| {
                        *fx.transaction_digest()
                    })
                })
            })
            .collect::<Vec<_>>();

        Ok(fx_results)
    }

    #[instrument(level = "trace", skip_all)]
    async fn multi_get_transactions_perpetual_checkpoints(
        &self,
        transaction_digests: &[TransactionDigest],
    ) -> IndexerResult<Vec<Option<CheckpointSequenceNumber>>> {
        let keys = transaction_digests
            .iter()
            .map(|digest| Key::TransactionToCheckpoint(*digest))
            .collect::<Vec<_>>();

        let fetches = self.multi_fetch(keys).await?;

        let results = fetches
            .iter()
            .zip(transaction_digests.iter())
            .map(|(fetch, digest)| {
                fetch
                    .as_ref()
                    .and_then(|bytes| deser::<_, CheckpointSequenceNumber>(&digest, bytes))
            })
            .collect::<Vec<_>>();

        Ok(results)
    }

    #[instrument(level = "trace", skip_all)]
    async fn multi_get_events_by_tx_digests(
        &self,
        transaction_digests: &[TransactionDigest],
    ) -> IndexerResult<Vec<Option<TransactionEvents>>> {
        let keys = transaction_digests
            .iter()
            .map(|digest| Key::EventsByTransactionDigest(*digest))
            .collect::<Vec<_>>();
        let fetches = self.multi_fetch(keys).await?;
        Ok(fetches
            .iter()
            .zip(transaction_digests.iter())
            .map(|(fetch, digest)| {
                fetch
                    .as_ref()
                    .and_then(|bytes| deser::<_, TransactionEvents>(&digest, bytes))
            })
            .collect::<Vec<_>>())
    }

    #[instrument(level = "trace", skip_all)]
    async fn multi_get_checkpoints_summaries_by_sequence_numbers(
        &self,
        checkpoint_sequence_numbers: &[CheckpointSequenceNumber],
    ) -> IndexerResult<Vec<Option<CertifiedCheckpointSummary>>> {
        let keys = checkpoint_sequence_numbers
            .iter()
            .map(|cp| Key::CheckpointSummary(*cp))
            .collect::<Vec<_>>();

        let fetches = self.multi_fetch(keys).await?;

        let summaries_results = fetches
            .iter()
            .zip(checkpoint_sequence_numbers.iter())
            .map(|(fetch, seq)| {
                fetch
                    .as_ref()
                    .and_then(|bytes| deser::<_, CertifiedCheckpointSummary>(seq, bytes))
            })
            .collect::<Vec<_>>();

        Ok(summaries_results)
    }

    #[instrument(level = "trace", skip_all)]
    async fn multi_get_checkpoints_contents(
        &self,
        checkpoint_sequence_numbers: &[CheckpointSequenceNumber],
    ) -> IndexerResult<Vec<Option<CheckpointContents>>> {
        let keys = checkpoint_sequence_numbers
            .iter()
            .map(|cp| Key::CheckpointContents(*cp))
            .collect::<Vec<_>>();

        let fetches = self.multi_fetch(keys).await?;

        let contents_results = fetches
            .iter()
            .zip(checkpoint_sequence_numbers.iter())
            .map(|(fetch, seq)| {
                fetch
                    .as_ref()
                    .and_then(|bytes| deser::<_, CheckpointContents>(seq, bytes))
            })
            .collect::<Vec<_>>();

        Ok(contents_results)
    }

    #[instrument(level = "trace", skip_all)]
    async fn multi_get_checkpoints_summaries_by_digests(
        &self,
        checkpoint_digests: &[CheckpointDigest],
    ) -> IndexerResult<Vec<Option<CertifiedCheckpointSummary>>> {
        let keys = checkpoint_digests
            .iter()
            .map(|cp| Key::CheckpointSummaryByDigest(*cp))
            .collect::<Vec<_>>();

        let fetches = self.multi_fetch(keys).await?;

        let summaries_by_digest_results = fetches
            .iter()
            .zip(checkpoint_digests.iter())
            .map(|(fetch, digest)| {
                fetch.as_ref().and_then(|bytes| {
                    deser_check_digest(digest, bytes, |s: &CertifiedCheckpointSummary| *s.digest())
                })
            })
            .collect::<Vec<_>>();

        Ok(summaries_by_digest_results)
    }

    #[instrument(level = "trace", skip_all)]
    async fn multi_get_objects(
        &self,
        object_refs: &[(ObjectID, SequenceNumber)],
    ) -> IndexerResult<Vec<Option<Object>>> {
        let keys = object_refs
            .iter()
            .map(|(object_id, version)| Key::ObjectKey(*object_id, *version))
            .collect::<Vec<_>>();

        let fetches = self.multi_fetch(keys).await?;

        let objects = fetches
            .iter()
            .zip(object_refs.iter())
            .map(|(fetch, object_ref)| fetch.as_ref().and_then(|bytes| deser(object_ref, bytes)))
            .collect::<Vec<_>>();

        Ok(objects)
    }
}
