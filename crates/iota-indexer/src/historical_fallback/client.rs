// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Module containing the client for interacting with the REST API KV server.

use std::str::FromStr;

use bytes::Bytes;
use futures::stream::{self, StreamExt};
use iota_storage::http_key_value_store::Key;
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
use reqwest::{
    Client, Url,
    header::{CONTENT_LENGTH, HeaderValue},
};
use serde::Deserialize;
use tap::TapFallible;
use tracing::{error, info, instrument, trace, warn};

use crate::errors::IndexerResult;

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
}

impl HttpRestKVClient {
    pub fn new(base_url: &str) -> IndexerResult<Self> {
        info!("creating HttpRestKVClient with base_url: {}", base_url);

        let client = Client::builder().http2_prior_knowledge().build()?;

        let base_url = if base_url.ends_with('/') {
            base_url.to_string()
        } else {
            format!("{base_url}/")
        };

        let base_url = Url::parse(&base_url)?;

        Ok(Self { base_url, client })
    }

    fn get_url(&self, key: &Key) -> IndexerResult<Url> {
        let (item_type, digest) = key.to_path_elements();
        let joined = self.base_url.join(&format!("{item_type}/{digest}"))?;
        Ok(Url::from_str(joined.as_str())?)
    }

    async fn multi_fetch(&self, uris: Vec<Key>) -> Vec<IndexerResult<Option<Bytes>>> {
        if uris.is_empty() {
            return Vec::new();
        }
        let len = uris.len();
        stream::iter(uris)
            .map(|url| self.fetch(url))
            // len must be greater than 0, otherwise it will enter a deadlock.
            .buffered(len)
            .collect()
            .await
    }

    async fn fetch(&self, key: Key) -> IndexerResult<Option<Bytes>> {
        let url = self.get_url(&key)?;

        trace!("fetching url: {url}");

        let resp = self.client.get(url.clone()).send().await?;
        trace!(
            "got response {} for url: {url}, len: {:?}",
            resp.status(),
            resp.headers()
                .get(CONTENT_LENGTH)
                .unwrap_or(&HeaderValue::from_static("0"))
        );

        // return None for non-2xx responses.
        if !resp.status().is_success() {
            return Ok(None);
        }

        let bytes = resp.bytes().await?;
        // map the bytes to Some only if non-empty.
        Ok((!bytes.is_empty()).then_some(bytes))
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

fn map_fetch<'a, K>(fetch: (&'a IndexerResult<Option<Bytes>>, &'a K)) -> Option<(&'a Bytes, &'a K)>
where
    K: std::fmt::Debug,
{
    let (fetch, key) = fetch;
    match fetch {
        Ok(Some(bytes)) => Some((bytes, key)),
        Ok(None) => None,
        Err(err) => {
            warn!("Error fetching key: {key:?}, error: {err:?}");
            None
        }
    }
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

        let fetches = self.multi_fetch(keys).await;
        let txn_results = fetches
            .iter()
            .zip(transaction_digests.iter())
            .map(map_fetch)
            .map(|maybe_bytes| {
                maybe_bytes.and_then(|(bytes, digest)| {
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

        let fetches = self.multi_fetch(keys).await;
        let fx_results = fetches
            .iter()
            .zip(transaction_digests.iter())
            .map(map_fetch)
            .map(|maybe_bytes| {
                maybe_bytes.and_then(|(bytes, digest)| {
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

        let fetches = self.multi_fetch(keys).await;

        let results = fetches
            .iter()
            .zip(transaction_digests.iter())
            .map(map_fetch)
            .map(|maybe_bytes| {
                maybe_bytes
                    .and_then(|(bytes, key)| deser::<_, CheckpointSequenceNumber>(&key, bytes))
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
        Ok(self
            .multi_fetch(keys)
            .await
            .iter()
            .zip(transaction_digests.iter())
            .map(map_fetch)
            .map(|maybe_bytes| {
                maybe_bytes.and_then(|(bytes, key)| deser::<_, TransactionEvents>(&key, bytes))
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

        let fetches = self.multi_fetch(keys).await;

        let summaries_results = fetches
            .iter()
            .zip(checkpoint_sequence_numbers.iter())
            .map(map_fetch)
            .map(|maybe_bytes| {
                maybe_bytes
                    .and_then(|(bytes, seq)| deser::<_, CertifiedCheckpointSummary>(seq, bytes))
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

        let fetches = self.multi_fetch(keys).await;

        let contents_results = fetches
            .iter()
            .zip(checkpoint_sequence_numbers.iter())
            .map(map_fetch)
            .map(|maybe_bytes| {
                maybe_bytes.and_then(|(bytes, seq)| deser::<_, CheckpointContents>(seq, bytes))
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

        let fetches = self.multi_fetch(keys).await;

        let summaries_by_digest_results = fetches
            .iter()
            .zip(checkpoint_digests.iter())
            .map(map_fetch)
            .map(|maybe_bytes| {
                maybe_bytes.and_then(|(bytes, digest)| {
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

        let fetches = self.multi_fetch(keys).await;

        let objects = fetches
            .iter()
            .zip(object_refs.iter())
            .map(map_fetch)
            .map(|maybe_bytes| maybe_bytes.and_then(|(bytes, object_ref)| deser(object_ref, bytes)))
            .collect::<Vec<_>>();

        Ok(objects)
    }
}
