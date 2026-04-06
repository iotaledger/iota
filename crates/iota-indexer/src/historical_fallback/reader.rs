// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Module containing the historical fallback reader implementation.
//!
//! This module provides a high-level client to interact with the historical
//! fallback storage. It enables integration with the
//! [`IndexerReader`](crate::read::IndexerReader) for fallback capabilities when
//! the indexer is unable to fetch data from the database, which is especially
//! useful when pruning is enabled.

use std::collections::HashMap;

use futures::future;
use iota_json_rpc_types::{CheckpointId, IotaEvent};
use iota_types::{
    base_types::{ObjectID, SequenceNumber},
    digests::TransactionDigest,
    effects::{TransactionEffects, TransactionEffectsAPI},
    event::EventID,
    full_checkpoint_content::CheckpointTransaction,
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContents, CheckpointSequenceNumber,
    },
    object::Object,
};
use itertools::{Either, Itertools, izip};
use prometheus::Registry;

use crate::{
    errors::{IndexerError, IndexerResult},
    historical_fallback::{
        client::{HttpRestKVClient, KeyValueStoreClient},
        convert::{
            HistoricalFallbackCheckpoint, HistoricalFallbackEvents, HistoricalFallbackTransaction,
        },
        metrics::HistoricalFallbackClientMetrics,
    },
    models::{
        checkpoints::StoredCheckpoint, objects::StoredObject, transactions::StoredTransaction,
    },
    read::PackageResolver,
};

/// Represents the Input objects of a transaction.
pub type InputObjects = Vec<Object>;
/// Represents the Output objects of a transaction.
pub type OutputObjects = Vec<Object>;

/// A high-level client to interact with the historical fallback storage.
///
/// Provides convenient methods to fetch data from the historical fallback
/// storage, with automatic conversions into types the Indexer uses when
/// fetching from the database. This enables integration with the
/// [`IndexerReader`](crate::read::IndexerReader) for fallback capabilities
/// when the indexer is unable to fetch data from the database, which is
/// especially useful when pruning is enabled.
#[derive(Clone)]
pub(crate) struct HistoricalFallbackReader {
    /// Client responsible for fetching data from the historical fallback
    /// storage through REST API interface.
    client: HttpRestKVClient,
    package_resolver: PackageResolver,
}

impl HistoricalFallbackReader {
    pub fn new(
        rest_kv_url: &str,
        cache_size: u64,
        package_resolver: PackageResolver,
        fallback_kv_multi_fetch_batch_size: usize,
        fallback_kv_concurrent_fetches: usize,
        registry: &Registry,
    ) -> IndexerResult<Self> {
        let client = HttpRestKVClient::new(
            rest_kv_url,
            cache_size,
            fallback_kv_multi_fetch_batch_size,
            fallback_kv_concurrent_fetches,
            HistoricalFallbackClientMetrics::new(registry),
        )?;
        Ok(Self {
            client,
            package_resolver,
        })
    }

    /// Resolves the input and output objects from a given transaction effects.
    pub(crate) async fn resolve_transaction_input_output_objects(
        &self,
        transaction_effects: &TransactionEffects,
    ) -> IndexerResult<(InputObjects, OutputObjects)> {
        let input_object_keys = transaction_effects.modified_at_versions();

        let output_object_keys = transaction_effects
            .all_changed_objects()
            .into_iter()
            .map(|((object_id, version, _object_digest), _owner, _kind)| (object_id, version))
            .collect::<Vec<(ObjectID, SequenceNumber)>>();

        let (raw_input_objects, raw_output_objects) = tokio::try_join!(
            self.client.multi_get_objects(&input_object_keys),
            self.client.multi_get_objects(&output_object_keys),
        )?;

        let input_objects = raw_input_objects
            .into_iter()
            .zip(&input_object_keys)
            .map(|(object, (object_id, version))| {
                object.ok_or_else(|| IndexerError::HistoricalFallbackObjectNotFound {
                    object_id: *object_id,
                    version: *version,
                })
            })
            .collect::<IndexerResult<Vec<Object>>>()?;

        let output_objects = raw_output_objects
            .into_iter()
            .zip(&output_object_keys)
            .map(|(object, (object_id, version))| {
                object.ok_or_else(|| IndexerError::HistoricalFallbackObjectNotFound {
                    object_id: *object_id,
                    version: *version,
                })
            })
            .collect::<IndexerResult<Vec<Object>>>()?;

        Ok((input_objects, output_objects))
    }

    /// Resolves the checkpoint summaries and contents by the provided
    /// transaction digests
    pub(crate) async fn resolve_checkpoints(
        &self,
        tx_digests: &[TransactionDigest],
    ) -> IndexerResult<HashMap<TransactionDigest, HistoricalFallbackCheckpoint>> {
        let tx_to_checkpoint_seq_num = self
            .client
            .multi_get_transactions_perpetual_checkpoints(tx_digests)
            .await?;

        // deduplicate checkpoint sequence numbers to avoid fetching the same summary
        // multiple times.
        let unique_seq_nums = tx_to_checkpoint_seq_num
            .iter()
            .flatten()
            .unique()
            .copied()
            .collect::<Vec<CheckpointSequenceNumber>>();

        let (summaries, contents) = tokio::try_join!(
            self.client
                .multi_get_checkpoints_summaries_by_sequence_numbers(&unique_seq_nums),
            self.client.multi_get_checkpoints_contents(&unique_seq_nums)
        )?;

        let checkpoints_map = summaries
            .into_iter()
            .zip(contents.into_iter())
            .filter_map(|(summary, contents)| {
                summary.and_then(|summary| contents.map(|contents| (summary.sequence_number, (summary, contents))))
            })
            .collect::<HashMap<CheckpointSequenceNumber, (CertifiedCheckpointSummary, CheckpointContents)>>();

        // map each tx digest to its checkpoint summary
        let summaries = tx_digests
            .iter()
            .zip(tx_to_checkpoint_seq_num)
            .filter_map(|(digest, seq_num)| {
                let seq = seq_num?;
                checkpoints_map
                    .get(&seq)
                    .cloned()
                    .map(|(summary, contents)| (*digest, (summary, contents)))
            })
            .collect();

        Ok(summaries)
    }

    /// Fetches a checkpoint by either a [`CheckpointSequenceNumber`] or
    /// [`CheckpointDigest`](iota_types::digests::CheckpointDigest).
    pub(crate) async fn checkpoint(
        &self,
        id: CheckpointId,
    ) -> IndexerResult<Option<StoredCheckpoint>> {
        let (summaries, contents) = match id {
            CheckpointId::SequenceNumber(sequence_number) => {
                let seq_nums = [sequence_number];
                tokio::try_join!(
                    self.client
                        .multi_get_checkpoints_summaries_by_sequence_numbers(&seq_nums),
                    self.client.multi_get_checkpoints_contents(&seq_nums)
                )?
            }
            CheckpointId::Digest(digest) => {
                let summaries = self
                    .client
                    .multi_get_checkpoints_summaries_by_digests(&[digest])
                    .await?;

                let Some(seq_num) = summaries
                    .first()
                    .and_then(|summary| summary.as_ref())
                    .map(|summary| summary.sequence_number)
                else {
                    return Ok(None);
                };

                let contents = self
                    .client
                    .multi_get_checkpoints_contents(&[seq_num])
                    .await?;

                (summaries, contents)
            }
        };

        let checkpoint = summaries
            .into_iter()
            .zip(contents)
            .next()
            .and_then(|(s, c)| Some(StoredCheckpoint::from((s?, c?))));

        Ok(checkpoint)
    }

    /// Fetches multiple checkpoints from the historical fallback storage.
    ///
    /// # NOTE
    /// `StoredCheckpoint.successful_tx_num` is hardcoded to 0, due to missing
    /// data in the historical fallback. It can be derived but the operations
    /// could be expensive. Can be added in future iterations.
    pub(crate) async fn checkpoints(
        &self,
        checkpoints: Vec<CheckpointSequenceNumber>,
    ) -> IndexerResult<Vec<Option<StoredCheckpoint>>> {
        if checkpoints.is_empty() {
            return Ok(vec![]);
        }

        let (summaries, contents) = tokio::try_join!(
            self.client
                .multi_get_checkpoints_summaries_by_sequence_numbers(&checkpoints),
            self.client.multi_get_checkpoints_contents(&checkpoints)
        )?;

        let checkpoints = summaries
            .into_iter()
            .zip(contents)
            .map(|(s, c)| Some(StoredCheckpoint::from((s?, c?))))
            .collect();

        Ok(checkpoints)
    }

    /// Fetches all events belonging to the provided transaction digest.
    pub(crate) async fn all_events(
        &self,
        tx_digest: TransactionDigest,
    ) -> IndexerResult<Vec<IotaEvent>> {
        let tx_digests = &[tx_digest];
        let (events, checkpoint_summaries) = tokio::try_join!(
            self.client.multi_get_events_by_tx_digests(tx_digests),
            self.resolve_checkpoints(tx_digests)
        )?;

        // check first if transaction exists, all valid transaction are part of a
        // checkpoint, if not found then the provided digest is invalid.
        let (summary, _) = checkpoint_summaries
            .get(&tx_digest)
            .cloned()
            .ok_or_else(|| {
                IndexerError::HistoricalFallbackStorageError(format!(
                    "transaction: {tx_digest} does not exist"
                ))
            })?;

        let Some(Some(events)) = events.into_iter().next() else {
            // transaction does not have associated events.
            return Ok(vec![]);
        };

        HistoricalFallbackEvents::new(events, summary)
            .into_iota_events(&self.package_resolver, tx_digest)
            .await
    }

    /// Fetches transactions from the provided transaction digests.
    pub(crate) async fn transactions(
        &self,
        tx_digests: &[TransactionDigest],
    ) -> IndexerResult<Vec<Option<StoredTransaction>>> {
        let (transactions, effects, events, checkpoints) = tokio::try_join!(
            self.client.multi_get_transactions(tx_digests),
            self.client.multi_get_effects(tx_digests),
            self.client.multi_get_events_by_tx_digests(tx_digests),
            self.resolve_checkpoints(tx_digests),
        )?;

        let futures =
            izip!(transactions, effects, events).map(|(transaction, effects, events)| async {
                let (Some(transaction), Some(effects)) = (transaction, effects) else {
                    return Ok(None);
                };

                let historical_checkpoint = checkpoints
                    .get(transaction.digest())
                    .cloned()
                    // if transaction exists but summary is not found this indicates a bug in data
                    // consistency in the KV Store.
                    .ok_or_else(|| {
                        IndexerError::HistoricalFallbackStorageError(format!(
                            "checkpoint summary and contents linked to transaction: {} not found",
                            transaction.digest()
                        ))
                    })?;

                let (input_objects, output_objects) = self
                    .resolve_transaction_input_output_objects(&effects)
                    .await?;

                let checkpoint_transaction = CheckpointTransaction {
                    transaction,
                    effects,
                    events,
                    input_objects,
                    output_objects,
                };

                HistoricalFallbackTransaction::new(checkpoint_transaction, historical_checkpoint)
                    .into_stored_transaction()
                    .await
                    .map(Some)
            });

        future::try_join_all(futures).await
    }

    /// Fetches objects by their ID and version from historical fallback
    /// storage.
    ///
    /// - If `before_version` is `false`, it looks for the exact version.
    /// - If `true`, it finds the latest version before the given one.
    ///
    /// # Note
    ///
    /// Currently only supports `before_version = false`.
    ///
    /// Support for `before_version = true` will be added once range scan is
    /// implemented on the KV REST API.
    pub(crate) async fn objects(
        &self,
        object_refs: &[(ObjectID, SequenceNumber)],
        before_version: bool,
    ) -> IndexerResult<Vec<Option<StoredObject>>> {
        if before_version {
            // TODO: Implement once range scan is available on KV REST API
            // For now, we cannot determine the correct previous version without it due to
            // non-contiguous object versioning:
            // https://docs.iota.org/developer/iota-101/objects/versioning#move-objects.
            return Ok(vec![None; object_refs.len()]);
        }

        let stored_objects = self
            .client
            .multi_get_objects(object_refs)
            .await?
            .into_iter()
            .map(|obj| obj.map(StoredObject::from))
            .collect();

        Ok(stored_objects)
    }

    /// Fetches transactions belonging to a specific checkpoint.
    ///
    /// Returns transactions in paginated form, supporting both ascending and
    /// descending order within the checkpoint.
    ///
    /// # Pagination Behavior
    ///
    /// | cursor     | descending | Result                                      |
    /// |------------|------------|---------------------------------------------|
    /// | `None`     | `false`    | Starts from first transaction in checkpoint |
    /// | `None`     | `true`     | Starts from last transaction in checkpoint  |
    /// | `Some(tx)` | `false`    | Starts after `tx`, ascending                |
    /// | `Some(tx)` | `true`     | Starts after `tx`, descending               |
    pub(crate) async fn checkpoint_transactions(
        &self,
        cursor: Option<TransactionDigest>,
        checkpoint_sequence_number: CheckpointSequenceNumber,
        limit: usize,
        is_descending: bool,
    ) -> IndexerResult<Vec<StoredTransaction>> {
        if limit == 0 {
            return Ok(vec![]);
        }

        let Some(contents) = self
            .client
            .multi_get_checkpoints_contents(&[checkpoint_sequence_number])
            .await?
            .into_iter()
            .next()
            .flatten()
        else {
            return Ok(vec![]);
        };

        let tx_digests = contents.iter().map(|b| b.transaction);

        // apply ordering
        let tx_digests = if is_descending {
            Either::Left(tx_digests.rev())
        } else {
            Either::Right(tx_digests)
        };

        // apply cursor: skip transactions until after the cursor.
        //
        // This relies on transactions being ordered within checkpoint contents,
        // so we can skip until we find the cursor, then skip the cursor itself.
        let tx_digests = if let Some(cursor) = cursor {
            Either::Left(
                tx_digests
                    .skip_while(move |digest| *digest != cursor)
                    .skip(1), // skip the cursor itself
            )
        } else {
            Either::Right(tx_digests)
        };

        // apply limit
        let tx_digests = tx_digests
            .into_iter()
            .take(limit)
            .collect::<Vec<TransactionDigest>>();

        let transactions = self.transactions(&tx_digests).await?;

        if transactions.iter().any(|tx| tx.is_none()) {
            return Err(IndexerError::HistoricalFallbackStorageError(format!(
                "KV doesn't have full transaction data for checkpoint {checkpoint_sequence_number}"
            )));
        }

        Ok(transactions.into_iter().flatten().collect::<Vec<_>>())
    }

    /// Fetches events for a specific transaction.
    ///
    /// Returns events emitted by the specified transaction, with support for
    /// cursor-based pagination and ordering.
    ///
    /// # Pagination Behavior
    ///
    /// Events are indexed by their position in the transaction (event_seq = 0,
    /// 1, 2, ...).
    ///
    /// | cursor      | descending | Result                   |
    /// |-------------|------------|--------------------------|
    /// | `None`      | `false`    | Starts from event_seq 0  |
    /// | `None`      | `true`     | Starts from last event   |
    /// | `Some(seq)` | `false`    | Starts after event_seq   |
    /// | `Some(seq)` | `true`     | Starts before event_seq  |
    pub(crate) async fn events(
        &self,
        tx_digest: TransactionDigest,
        cursor: Option<EventID>,
        limit: usize,
        descending_order: bool,
    ) -> IndexerResult<Vec<IotaEvent>> {
        if limit == 0 {
            return Ok(vec![]);
        }

        // validate cursor if provided
        let start_seq = if let Some(cursor) = cursor {
            if cursor.tx_digest != tx_digest {
                return Err(IndexerError::InvalidArgument(format!(
                    "Cursor tx_digest {} does not match requested tx_digest {tx_digest}",
                    cursor.tx_digest
                )));
            }
            Some(cursor.event_seq)
        } else {
            None
        };

        let events = self.all_events(tx_digest).await?;

        // apply ordering, cursor, and limit
        let events = if descending_order {
            events
                .into_iter()
                .enumerate()
                .rev() // reverse for descending
                .filter(|(idx, _)| start_seq.is_none_or(|seq| (*idx as u64) < seq))
                .take(limit)
                .map(|(_, event)| event)
                .collect()
        } else {
            events
                .into_iter()
                .enumerate()
                .filter(|(idx, _)| start_seq.is_none_or(|seq| (*idx as u64) > seq))
                .take(limit)
                .map(|(_, event)| event)
                .collect()
        };

        Ok(events)
    }
}
