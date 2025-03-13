// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{str::FromStr, sync::Arc};

use anyhow;
use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::{self, StreamExt};
use iota_types::{
    base_types::{ObjectID, SequenceNumber, VersionNumber},
    digests::{CheckpointDigest, TransactionDigest},
    effects::{TransactionEffects, TransactionEffectsAPI, TransactionEvents},
    error::{IotaError, IotaResult},
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContents, CheckpointSequenceNumber,
    },
    object::Object,
    storage::ObjectKey,
    transaction::Transaction,
};
use reqwest::{
    Client, Url,
    header::{CONTENT_LENGTH, HeaderValue},
};
use serde::{Deserialize, Serialize};
use tap::TapFallible;
use tracing::{error, info, instrument, trace, warn};

use crate::{
    key_value_store::{
        KVStoreTransactionData, TransactionKeyValueStore, TransactionKeyValueStoreTrait,
    },
    key_value_store_metrics::KeyValueStoreMetrics,
};

pub struct HttpKVStore {
    base_url: Url,
    client: Client,
}

pub fn encode_digest<T: AsRef<[u8]>>(digest: &T) -> String {
    base64_url::encode(digest)
}

// for non-digest keys, we need a tag to make sure we don't have collisions
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaggedKey {
    CheckpointSequenceNumber(CheckpointSequenceNumber),
}

pub fn encoded_tagged_key(key: &TaggedKey) -> String {
    let bytes = bcs::to_bytes(key).expect("failed to serialize key");
    base64_url::encode(&bytes)
}

pub fn encode_object_key(object_id: &ObjectID, version: &VersionNumber) -> String {
    let bytes =
        bcs::to_bytes(&ObjectKey(*object_id, *version)).expect("failed to serialize object key");
    base64_url::encode(&bytes)
}

trait IntoIotaResult<T> {
    fn into_iota_result(self) -> IotaResult<T>;
}

impl<T, E> IntoIotaResult<T> for Result<T, E>
where
    E: std::error::Error,
{
    fn into_iota_result(self) -> IotaResult<T> {
        self.map_err(|e| IotaError::Storage(e.to_string()))
    }
}

/// Represents the supported items the REST API accepts when fetching the data
/// based on Digest or Sequence number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, strum::EnumString, strum::Display)]
pub enum ItemType {
    #[strum(serialize = "tx")]
    #[serde(rename = "tx")]
    Transaction,
    #[strum(serialize = "fx")]
    #[serde(rename = "fx")]
    TransactionEffects,
    #[strum(serialize = "cc")]
    #[serde(rename = "cc")]
    CheckpointContents,
    #[strum(serialize = "cs")]
    #[serde(rename = "cs")]
    CheckpointSummary,
    #[strum(serialize = "tx2c")]
    #[serde(rename = "tx2c")]
    TransactionToCheckpoint,
    #[strum(serialize = "ob")]
    #[serde(rename = "ob")]
    Object,
    #[strum(serialize = "evtx")]
    #[serde(rename = "evtx")]
    EventTransactionDigest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Transaction(TransactionDigest),
    TransactionEffects(TransactionDigest),
    CheckpointContents(CheckpointSequenceNumber),
    CheckpointSummary(CheckpointSequenceNumber),
    CheckpointSummaryByDigest(CheckpointDigest),
    TransactionToCheckpoint(TransactionDigest),
    ObjectKey(ObjectID, VersionNumber),
    EventsByTransactionDigest(TransactionDigest),
}

impl Key {
    // Create a [`Key`] instance based on the provided item type and
    /// [`base64_url`] encoded string.
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::str::FromStr;
    ///
    /// use iota_storage::http_key_value_store::Key;
    /// use iota_types::digests::TransactionDigest;
    ///
    /// let key = Key::new("tx", "7jb54RvJduLj9HdV9L41UJqZ5KWdzYY2rl1eL8AVl9o").unwrap();
    /// assert_eq!(
    ///     key,
    ///     Key::Transaction(
    ///         TransactionDigest::from_str("H2tetNL3CfroDF3iJNA7wFo6oRQiJedGTeykZi6HAGqP").unwrap()
    ///     )
    /// );
    /// ```
    pub fn new(item_type: &str, encoded_key: &str) -> anyhow::Result<Self> {
        let item_type =
            ItemType::from_str(item_type).map_err(|e| anyhow::anyhow!("invalid item type: {e}"))?;
        let decoded_key = base64_url::decode(encoded_key)
            .map_err(|err| anyhow::anyhow!("invalid base64 url string: {err}"))?;

        match item_type {
            ItemType::Transaction => Ok(Key::Transaction(TransactionDigest::try_from(
                decoded_key.as_slice(),
            )?)),
            ItemType::TransactionEffects => Ok(Key::TransactionEffects(
                TransactionDigest::try_from(decoded_key.as_slice())?,
            )),
            ItemType::CheckpointContents => {
                let tagged_key = bcs::from_bytes(&decoded_key).map_err(|err| {
                    anyhow::anyhow!("failed to deserialize checkpoint sequence number: {err}")
                })?;
                match tagged_key {
                    TaggedKey::CheckpointSequenceNumber(seq) => Ok(Key::CheckpointContents(seq)),
                }
            }
            ItemType::CheckpointSummary => {
                // first try to decode as digest, otherwise try to decode as tagged key
                match CheckpointDigest::try_from(decoded_key.clone()) {
                    Err(_) => {
                        let tagged_key = bcs::from_bytes(&decoded_key).map_err(|err| {
                            anyhow::anyhow!(
                                "failed to deserialize checkpoint sequence number: {err}"
                            )
                        })?;
                        match tagged_key {
                            TaggedKey::CheckpointSequenceNumber(seq) => {
                                Ok(Key::CheckpointSummary(seq))
                            }
                        }
                    }
                    Ok(cs_digest) => Ok(Key::CheckpointSummaryByDigest(cs_digest)),
                }
            }
            ItemType::TransactionToCheckpoint => Ok(Key::TransactionToCheckpoint(
                TransactionDigest::try_from(decoded_key.as_slice())?,
            )),
            ItemType::Object => {
                let object_key: ObjectKey = bcs::from_bytes(&decoded_key)
                    .map_err(|err| anyhow::anyhow!("failed to deserialize object key: {err}"))?;

                Ok(Key::ObjectKey(object_key.0, object_key.1))
            }
            ItemType::EventTransactionDigest => Ok(Key::EventsByTransactionDigest(
                TransactionDigest::try_from(decoded_key.as_slice())?,
            )),
        }
    }

    /// Get the REST API resource type.
    ///
    /// This method returns the corresponding resource type string
    /// for a given `Key` variant.
    ///
    /// This is used to construct the REST API route,
    /// typically in the format `/:item_type/:digest`.
    ///
    /// # Example
    /// ```rust
    /// use iota_storage::http_key_value_store::{ItemType, Key};
    /// use iota_types::digests::TransactionDigest;
    ///
    /// let item_type = Key::CheckpointContents(1).item_type();
    /// assert_eq!(item_type, ItemType::CheckpointContents);
    /// let item_type = Key::Transaction(TransactionDigest::random()).item_type();
    /// assert_eq!(item_type, ItemType::Transaction);
    /// ```
    pub fn item_type(&self) -> ItemType {
        match self {
            Key::Transaction(_) => ItemType::Transaction,
            Key::TransactionEffects(_) => ItemType::TransactionEffects,
            Key::CheckpointContents(_) => ItemType::CheckpointContents,
            Key::CheckpointSummary(_) | Key::CheckpointSummaryByDigest(_) => {
                ItemType::CheckpointSummary
            }
            Key::TransactionToCheckpoint(_) => ItemType::TransactionToCheckpoint,
            Key::ObjectKey(_, _) => ItemType::Object,
            Key::EventsByTransactionDigest(_) => ItemType::EventTransactionDigest,
        }
    }

    /// Returns a tuple containing the resource type and the encoded key.
    ///
    /// This is used to construct the REST API route, typically in the format
    /// `/:item_type/:digest`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use iota_storage::http_key_value_store::{
    ///     ItemType, Key, TaggedKey, encode_digest, encode_object_key, encoded_tagged_key,
    /// };
    /// use iota_types::digests::TransactionDigest;
    ///
    /// let tx_digest = TransactionDigest::random();
    /// // encode the tx_digest as base64 url
    /// let expected_encoded_digest = encode_digest(&tx_digest);
    /// let key = Key::Transaction(tx_digest);
    /// let (resource_type, encoded_key_digest) = key.to_path_elements();
    /// assert_eq!(resource_type, ItemType::Transaction);
    /// assert_eq!(encoded_key_digest, expected_encoded_digest);
    ///
    /// let chk_seq_num = 123;
    /// let key = Key::CheckpointSummary(chk_seq_num);
    /// // encode the checkpoint sequence number as base64 url
    /// let expected_encoded_seq_num =
    ///     encoded_tagged_key(&TaggedKey::CheckpointSequenceNumber(chk_seq_num));
    /// let (resource_type, encoded_key_digest) = key.to_path_elements();
    /// assert_eq!(resource_type, ItemType::CheckpointSummary);
    /// assert_eq!(encoded_key_digest, expected_encoded_seq_num);
    /// ```
    pub fn to_path_elements(&self) -> (ItemType, String) {
        let encoded_key_digest = match self {
            Key::Transaction(digest) => encode_digest(digest),
            Key::TransactionEffects(digest) => encode_digest(digest),
            Key::CheckpointContents(seq) => {
                encoded_tagged_key(&TaggedKey::CheckpointSequenceNumber(*seq))
            }
            Key::CheckpointSummary(seq) => {
                encoded_tagged_key(&TaggedKey::CheckpointSequenceNumber(*seq))
            }
            Key::CheckpointSummaryByDigest(digest) => encode_digest(digest),
            Key::TransactionToCheckpoint(digest) => encode_digest(digest),
            Key::ObjectKey(object_id, version) => encode_object_key(object_id, version),
            Key::EventsByTransactionDigest(digest) => encode_digest(digest),
        };

        (self.item_type(), encoded_key_digest)
    }
}

#[derive(Clone, Debug)]
enum Value {
    Tx(Box<Transaction>),
    Fx(Box<TransactionEffects>),
    Events(Box<TransactionEvents>),
    CheckpointContents(Box<CheckpointContents>),
    CheckpointSummary(Box<CertifiedCheckpointSummary>),
    TxToCheckpoint(CheckpointSequenceNumber),
}

impl HttpKVStore {
    pub fn new_kv(
        base_url: &str,
        metrics: Arc<KeyValueStoreMetrics>,
    ) -> IotaResult<TransactionKeyValueStore> {
        let inner = Arc::new(Self::new(base_url)?);
        Ok(TransactionKeyValueStore::new("http", metrics, inner))
    }

    pub fn new(base_url: &str) -> IotaResult<Self> {
        info!("creating HttpKVStore with base_url: {}", base_url);

        let client = Client::builder().http2_prior_knowledge().build().unwrap();

        let base_url = if base_url.ends_with('/') {
            base_url.to_string()
        } else {
            format!("{}/", base_url)
        };

        let base_url = Url::parse(&base_url).into_iota_result()?;

        Ok(Self { base_url, client })
    }

    fn get_url(&self, key: &Key) -> IotaResult<Url> {
        let (item_type, digest) = key.to_path_elements();
        let joined = self
            .base_url
            .join(&format!("{item_type}/{digest}"))
            .into_iota_result()?;
        Url::from_str(joined.as_str()).into_iota_result()
    }

    async fn multi_fetch(&self, uris: Vec<Key>) -> Vec<IotaResult<Option<Bytes>>> {
        let uris_vec = uris.to_vec();
        let fetches = stream::iter(uris_vec.into_iter().map(|url| self.fetch(url)));
        fetches.buffered(uris.len()).collect::<Vec<_>>().await
    }

    async fn fetch(&self, key: Key) -> IotaResult<Option<Bytes>> {
        let url = self.get_url(&key)?;
        trace!("fetching url: {}", url);
        let resp = self
            .client
            .get(url.clone())
            .send()
            .await
            .into_iota_result()?;
        trace!(
            "got response {} for url: {}, len: {:?}",
            url,
            resp.status(),
            resp.headers()
                .get(CONTENT_LENGTH)
                .unwrap_or(&HeaderValue::from_static("0"))
        );
        // return None if 400
        if resp.status().is_success() {
            resp.bytes().await.map(Some).into_iota_result()
        } else {
            Ok(None)
        }
    }
}

fn deser<K, T>(key: &K, bytes: &[u8]) -> Option<T>
where
    K: std::fmt::Debug,
    T: for<'de> Deserialize<'de>,
{
    bcs::from_bytes(bytes)
        .tap_err(|e| warn!("Error deserializing data for key {:?}: {:?}", key, e))
        .ok()
}

fn map_fetch<'a, K>(fetch: (&'a IotaResult<Option<Bytes>>, &'a K)) -> Option<(&'a Bytes, &'a K)>
where
    K: std::fmt::Debug,
{
    let (fetch, key) = fetch;
    match fetch {
        Ok(Some(bytes)) => Some((bytes, key)),
        Ok(None) => None,
        Err(err) => {
            warn!("Error fetching key: {:?}, error: {:?}", key, err);
            None
        }
    }
}

fn multi_split_slice<'a, T>(slice: &'a [T], lengths: &'a [usize]) -> Vec<&'a [T]> {
    let mut start = 0;
    lengths
        .iter()
        .map(|length| {
            let end = start + length;
            let result = &slice[start..end];
            start = end;
            result
        })
        .collect()
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
            error!(
                "Digest mismatch - expected: {:?}, got: {:?}",
                digest, expected_digest,
            );
            None
        }
    })
}

#[async_trait]
impl TransactionKeyValueStoreTrait for HttpKVStore {
    #[instrument(level = "trace", skip_all)]
    async fn multi_get(
        &self,
        transaction_keys: &[TransactionDigest],
        effects_keys: &[TransactionDigest],
    ) -> IotaResult<KVStoreTransactionData> {
        let num_txns = transaction_keys.len();
        let num_effects = effects_keys.len();

        let keys = transaction_keys
            .iter()
            .map(|tx| Key::Transaction(*tx))
            .chain(effects_keys.iter().map(|fx| Key::TransactionEffects(*fx)))
            .collect::<Vec<_>>();

        let fetches = self.multi_fetch(keys).await;
        let txn_slice = fetches[..num_txns].to_vec();
        let fx_slice = fetches[num_txns..num_txns + num_effects].to_vec();

        let txn_results = txn_slice
            .iter()
            .take(num_txns)
            .zip(transaction_keys.iter())
            .map(map_fetch)
            .map(|maybe_bytes| {
                maybe_bytes.and_then(|(bytes, digest)| {
                    deser_check_digest(digest, bytes, |tx: &Transaction| *tx.digest())
                })
            })
            .collect::<Vec<_>>();

        let fx_results = fx_slice
            .iter()
            .take(num_effects)
            .zip(effects_keys.iter())
            .map(map_fetch)
            .map(|maybe_bytes| {
                maybe_bytes.and_then(|(bytes, digest)| {
                    deser_check_digest(digest, bytes, |fx: &TransactionEffects| {
                        *fx.transaction_digest()
                    })
                })
            })
            .collect::<Vec<_>>();

        Ok((txn_results, fx_results))
    }

    #[instrument(level = "trace", skip_all)]
    async fn multi_get_checkpoints(
        &self,
        checkpoint_summaries: &[CheckpointSequenceNumber],
        checkpoint_contents: &[CheckpointSequenceNumber],
        checkpoint_summaries_by_digest: &[CheckpointDigest],
    ) -> IotaResult<(
        Vec<Option<CertifiedCheckpointSummary>>,
        Vec<Option<CheckpointContents>>,
        Vec<Option<CertifiedCheckpointSummary>>,
    )> {
        let keys = checkpoint_summaries
            .iter()
            .map(|cp| Key::CheckpointSummary(*cp))
            .chain(
                checkpoint_contents
                    .iter()
                    .map(|cp| Key::CheckpointContents(*cp)),
            )
            .chain(
                checkpoint_summaries_by_digest
                    .iter()
                    .map(|cp| Key::CheckpointSummaryByDigest(*cp)),
            )
            .collect::<Vec<_>>();

        let summaries_len = checkpoint_summaries.len();
        let contents_len = checkpoint_contents.len();
        let summaries_by_digest_len = checkpoint_summaries_by_digest.len();

        let fetches = self.multi_fetch(keys).await;

        let input_slices = [summaries_len, contents_len, summaries_by_digest_len];

        let result_slices = multi_split_slice(&fetches, &input_slices);

        let summaries_results = result_slices[0]
            .iter()
            .zip(checkpoint_summaries.iter())
            .map(map_fetch)
            .map(|maybe_bytes| {
                maybe_bytes
                    .and_then(|(bytes, seq)| deser::<_, CertifiedCheckpointSummary>(seq, bytes))
            })
            .collect::<Vec<_>>();

        let contents_results = result_slices[1]
            .iter()
            .zip(checkpoint_contents.iter())
            .map(map_fetch)
            .map(|maybe_bytes| {
                maybe_bytes.and_then(|(bytes, seq)| deser::<_, CheckpointContents>(seq, bytes))
            })
            .collect::<Vec<_>>();

        let summaries_by_digest_results = result_slices[2]
            .iter()
            .zip(checkpoint_summaries_by_digest.iter())
            .map(map_fetch)
            .map(|maybe_bytes| {
                maybe_bytes.and_then(|(bytes, digest)| {
                    deser_check_digest(digest, bytes, |s: &CertifiedCheckpointSummary| *s.digest())
                })
            })
            .collect::<Vec<_>>();

        Ok((
            summaries_results,
            contents_results,
            summaries_by_digest_results,
        ))
    }

    #[instrument(level = "trace", skip_all)]
    async fn get_transaction_perpetual_checkpoint(
        &self,
        digest: TransactionDigest,
    ) -> IotaResult<Option<CheckpointSequenceNumber>> {
        let key = Key::TransactionToCheckpoint(digest);
        self.fetch(key).await.map(|maybe| {
            maybe.and_then(|bytes| deser::<_, CheckpointSequenceNumber>(&key, bytes.as_ref()))
        })
    }

    #[instrument(level = "trace", skip_all)]
    async fn get_object(
        &self,
        object_id: ObjectID,
        version: SequenceNumber,
    ) -> IotaResult<Option<Object>> {
        let key = Key::ObjectKey(object_id, version);
        self.fetch(key)
            .await
            .map(|maybe| maybe.and_then(|bytes| deser::<_, Object>(&key, bytes.as_ref())))
    }

    #[instrument(level = "trace", skip_all)]
    async fn multi_get_transactions_perpetual_checkpoints(
        &self,
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Option<CheckpointSequenceNumber>>> {
        let keys = digests
            .iter()
            .map(|digest| Key::TransactionToCheckpoint(*digest))
            .collect::<Vec<_>>();

        let fetches = self.multi_fetch(keys).await;

        let results = fetches
            .iter()
            .zip(digests.iter())
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
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Option<TransactionEvents>>> {
        let keys = digests
            .iter()
            .map(|digest| Key::EventsByTransactionDigest(*digest))
            .collect::<Vec<_>>();
        Ok(self
            .multi_fetch(keys)
            .await
            .iter()
            .zip(digests.iter())
            .map(map_fetch)
            .map(|maybe_bytes| {
                maybe_bytes.and_then(|(bytes, key)| deser::<_, TransactionEvents>(&key, bytes))
            })
            .collect::<Vec<_>>())
    }
}
