// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{pin::Pin, sync::Arc};

use anyhow::Result;
use futures::StreamExt;
use grpc_ledger_service::checkpoint_data::Progress;
use iota_grpc_types::{
    field::FieldMaskTree,
    proto::timestamp_ms_to_proto,
    v1::{
        checkpoint as grpc_checkpoint, event as grpc_event,
        ledger_service::{self as grpc_ledger_service},
        transaction as grpc_transaction,
    },
};
use iota_node_storage::GrpcStateReader;
use iota_sdk_types::{
    Address, CheckpointContents, CheckpointDigest, ObjectId, StructTag, TransactionDigest,
    TransactionEffects, TransactionEvents, TypeTag, Version,
};
use iota_types::{
    base_types::VersionNumber,
    effects::TransactionEffectsAPI,
    full_checkpoint_content::{
        CheckpointData as IotaTypesCheckpointData,
        CheckpointTransaction as IotaTypesCheckpointTransaction,
    },
    messages_checkpoint::CertifiedCheckpointSummary,
    object::Object,
    storage::error::Kind,
};
use prometheus_filtered::IntGauge;
use prost::Message;
use tokio::sync::{
    OwnedSemaphorePermit, Semaphore,
    broadcast::{Receiver, Sender, error::RecvError},
};
use tokio_util::sync::CancellationToken;
use tonic::Status;
use tracing::debug;

use crate::{
    error::RpcError,
    merge::Merge,
    metrics::{CheckpointSizeTracker, RequestMetrics, ResponseItemKind},
};

/// Flags indicating which optional transaction fields to fetch from storage.
/// Derived from a `FieldMaskTree` to skip unnecessary storage reads.
#[derive(Debug, Clone, Copy, Default)]
pub struct TransactionReadFields {
    pub include_transaction: bool,
    pub include_signatures: bool,
    pub include_effects: bool,
    pub include_events: bool,
    pub include_checkpoint: bool,
    pub include_timestamp: bool,
    pub include_input_objects: bool,
    pub include_output_objects: bool,
    pub include_balance_changes: bool,
    pub include_object_changes: bool,
}

impl TransactionReadFields {
    /// Derive which fields to fetch from an `ExecutedTransaction` field mask.
    pub fn from_mask(mask: &FieldMaskTree) -> Self {
        use iota_grpc_types::v1::transaction::ExecutedTransaction;

        Self {
            include_transaction: mask.contains(ExecutedTransaction::TRANSACTION_FIELD.name),
            include_signatures: mask.contains(ExecutedTransaction::SIGNATURES_FIELD.name),
            include_effects: mask.contains(ExecutedTransaction::EFFECTS_FIELD.name),
            include_events: mask.contains(ExecutedTransaction::EVENTS_FIELD.name),
            include_checkpoint: mask.contains(ExecutedTransaction::CHECKPOINT_FIELD.name),
            include_timestamp: mask.contains(ExecutedTransaction::TIMESTAMP_FIELD.name),
            include_input_objects: mask.contains(ExecutedTransaction::INPUT_OBJECTS_FIELD.name),
            include_output_objects: mask.contains(ExecutedTransaction::OUTPUT_OBJECTS_FIELD.name),
            include_balance_changes: mask.contains(ExecutedTransaction::BALANCE_CHANGES_FIELD.name),
            include_object_changes: mask.contains(ExecutedTransaction::OBJECT_CHANGES_FIELD.name),
        }
    }
}

pub type GetObjectsStream = Pin<Box<dyn futures::Stream<Item = ObjectsStreamResult> + Send>>;
pub type GetTransactionsStream =
    Pin<Box<dyn futures::Stream<Item = TransactionsStreamResult> + Send>>;

/// Server streaming response type for the GetCheckpoint method.
pub type GetCheckpointStream = Pin<Box<dyn futures::Stream<Item = CheckpointStreamResult> + Send>>;

/// Server streaming response type for the StreamCheckpoints method.
pub type StreamCheckpointsStream =
    Pin<Box<dyn futures::Stream<Item = CheckpointStreamResult> + Send>>;

/// Broadcasts checkpoint data to subscribers, capping concurrent subscribers.
#[derive(Clone)]
pub struct GrpcCheckpointDataBroadcaster {
    sender: Sender<Arc<IotaTypesCheckpointData>>,
    /// Semaphore enforcing the concurrent-subscriber cap. Each successful
    /// `subscribe()` acquires one permit; dropping the returned
    /// [`SubscriberGuard`] releases it. This makes the cap atomic (no
    /// check-then-subscribe race).
    subscription_semaphore: Arc<Semaphore>,
    /// Optional gauge tracking the number of active subscribers. Incremented
    /// on `subscribe()`, decremented when the subscriber's
    /// [`SubscriberGuard`] drops — so the metric is always in sync with
    /// actual subscriber count, including client disconnects between
    /// broadcasts.
    inflight_subscribers: Option<IntGauge>,
}

/// A broadcast [`Receiver`] bundled with the [`SubscriberGuard`] holding
/// its slot in the subscriber cap. Bundling them at the `subscribe()`
/// boundary makes it impossible for callers to acquire a receiver without
/// also holding the cap/gauge lifetime.
#[must_use = "dropping the SubscribedReceiver immediately releases the subscriber slot"]
pub struct SubscribedReceiver {
    pub(crate) rx: Receiver<Arc<IotaTypesCheckpointData>>,
    pub(crate) guard: SubscriberGuard,
}

/// RAII guard that holds one slot in the subscriber cap and keeps the
/// inflight gauge in sync: the gauge is incremented on construction and
/// decremented on drop.
pub struct SubscriberGuard {
    _permit: OwnedSemaphorePermit,
    gauge: Option<IntGauge>,
}

impl SubscriberGuard {
    fn new(permit: OwnedSemaphorePermit, gauge: Option<IntGauge>) -> Self {
        if let Some(g) = &gauge {
            g.inc();
        }
        Self {
            _permit: permit,
            gauge,
        }
    }
}

impl Drop for SubscriberGuard {
    fn drop(&mut self) {
        if let Some(gauge) = &self.gauge {
            gauge.dec();
        }
    }
}

impl GrpcCheckpointDataBroadcaster {
    pub fn new(
        sender: Sender<Arc<IotaTypesCheckpointData>>,
        max_subscribers: usize,
        inflight_subscribers: Option<IntGauge>,
    ) -> Self {
        Self {
            sender,
            subscription_semaphore: Arc::new(Semaphore::new(max_subscribers)),
            inflight_subscribers,
        }
    }

    /// Subscribe to checkpoint data broadcasts, enforcing the configured cap
    /// on concurrent subscribers.
    ///
    /// Returns `None` when the cap has been reached. Callers should surface
    /// this as `Unavailable` to the client (transient capacity, retryable).
    pub fn subscribe(&self) -> Option<SubscribedReceiver> {
        let permit = self
            .subscription_semaphore
            .clone()
            .try_acquire_owned()
            .ok()?;
        let rx = self.sender.subscribe();
        let guard = SubscriberGuard::new(permit, self.inflight_subscribers.clone());
        Some(SubscribedReceiver { rx, guard })
    }

    /// Get the number of active broadcast receivers.
    ///
    /// Counts every receiver on the underlying `broadcast::Sender`, including
    /// any internal subscribers that did not go through [`subscribe`] and are
    /// therefore not tracked by the subscriber cap or `inflight_subscribers`
    /// gauge. Use this when deciding whether to send on the channel; use the
    /// gauge when reporting externally-subscribed stream count.
    ///
    /// [`subscribe`]: Self::subscribe
    pub fn receiver_count(&self) -> usize {
        self.sender.receiver_count()
    }

    /// Send with integrated tracing and error handling
    pub fn send_traced(&self, data: &IotaTypesCheckpointData) {
        // Only send if there are active subscribers
        if self.receiver_count() == 0 {
            return;
        }

        match self.sender.send(Arc::new(data.clone())) {
            Ok(_) => {
                debug!(
                    "Sent checkpoint data #{} to {} gRPC subscriber(s)",
                    data.checkpoint_summary.data().sequence_number,
                    self.receiver_count()
                );
            }
            Err(_) => {
                debug!(
                    "No gRPC clients subscribed for checkpoint data #{}",
                    data.checkpoint_summary.data().sequence_number
                );
            }
        }
    }
}

// Type aliases and utility types
pub type ObjectsStreamResult = Result<grpc_ledger_service::GetObjectsResponse, Status>;
pub type TransactionsStreamResult = Result<grpc_ledger_service::GetTransactionsResponse, Status>;
pub type CheckpointStreamResult = Result<grpc_ledger_service::CheckpointData, Status>;

// Iterator item types for state reader methods.
//
// These mirror the `iota_types::storage` item types but use `anyhow::Result`
// so that different storage backends (RocksDB, mock, simulacrum) can map
// their concrete errors into a uniform error type.

/// A dynamic-field index key (parent + field_id).
pub type DynamicFieldIterItem = anyhow::Result<iota_types::storage::DynamicFieldKey>;

pub use iota_types::storage::OwnedObjectCursor;

/// An owned-object together with a seek cursor for the position it
/// occupies in the index.
///
/// Carries the full key components so that page tokens can encode an exact
/// seek position.
pub type OwnedObjectIterItem = anyhow::Result<(
    iota_types::storage::AccountOwnedObjectInfo,
    iota_types::storage::OwnedObjectCursor,
)>;

/// A package-version index entry (key + storage info).
pub type PackageVersionIterItem = anyhow::Result<(
    iota_types::storage::PackageVersionKey,
    iota_types::storage::PackageVersionInfo,
)>;

/// Result of [`GrpcReader::match_checkpoint_filter_or_report_progress`].
enum FilterCheckResult {
    /// The checkpoint contains matching data; proceed with full processing.
    Matched,
    /// The checkpoint should be skipped, with an optional progress message to
    /// yield before returning.
    Skipped(Option<grpc_ledger_service::CheckpointData>),
}

/// Get the latest checkpoint sequence number from a `GrpcStateReader`.
///
/// Handles the `Kind::Missing` error during server startup (when no checkpoints
/// have been executed yet) by returning `Ok(None)`.
fn latest_checkpoint_seq(reader: &dyn GrpcStateReader) -> anyhow::Result<Option<u64>> {
    match reader.try_get_latest_checkpoint() {
        Ok(checkpoint) => Ok(Some(checkpoint.sequence_number())),
        Err(e) => match e.kind() {
            Kind::Missing => Ok(None),
            _ => Err(anyhow::anyhow!(
                "Storage error getting latest checkpoint: {e}"
            )),
        },
    }
}

/// Compose checkpoint summary and contents from two storage calls.
fn checkpoint_summary_and_contents(
    reader: &dyn GrpcStateReader,
    seq: u64,
) -> anyhow::Result<Option<(CertifiedCheckpointSummary, CheckpointContents)>> {
    let summary = reader
        .try_get_checkpoint_by_sequence_number(seq)
        .map_err(anyhow::Error::from)?;
    let contents = reader
        .try_get_checkpoint_contents_by_sequence_number(seq)
        .map_err(anyhow::Error::from)?;
    match (summary, contents) {
        (Some(s), Some(c)) => Ok(Some((CertifiedCheckpointSummary::from(s), c))),
        _ => Ok(None),
    }
}

/// Iterates the transactions of a live checkpoint one at a time, cloning
/// each only when the stream reaches it, instead of cloning the whole list
/// up front. A stream that the client does not read then holds only the
/// transaction it last cloned, not the rest of the checkpoint too.
fn live_checkpoint_transaction_stream(
    item: Arc<IotaTypesCheckpointData>,
) -> impl futures::Stream<Item = anyhow::Result<IotaTypesCheckpointTransaction>> {
    futures::stream::iter(0..item.transactions.len())
        .map(move |i| Ok::<_, anyhow::Error>(item.transactions[i].clone()))
}

/// Central gRPC data reader wrapping a [`GrpcStateReader`].
#[derive(Clone)]
pub struct GrpcReader {
    state_reader: Arc<dyn iota_node_storage::GrpcStateReader>,
    server_version: Option<String>,
}

impl GrpcReader {
    /// Get a reference to the gRPC indexes, returning an error if they are
    /// not available (i.e. disabled or not yet initialised). The error
    /// renders as `FailedPrecondition`, so clients can tell an unsupported
    /// node configuration from a server fault.
    fn require_indexes(
        &self,
    ) -> Result<&dyn iota_node_storage::GrpcIndexes, crate::error::MissingIndexesError> {
        self.state_reader
            .grpc_indexes()
            .ok_or(crate::error::MissingIndexesError)
    }

    pub fn new(
        state_reader: Arc<dyn iota_node_storage::GrpcStateReader>,
        server_version: Option<String>,
    ) -> Self {
        Self {
            state_reader,
            server_version,
        }
    }

    pub fn server_version(&self) -> Option<String> {
        self.server_version.clone()
    }

    pub fn state_reader(&self) -> Arc<dyn iota_node_storage::GrpcStateReader> {
        self.state_reader.clone()
    }

    pub fn get_chain_identifier(&self) -> anyhow::Result<iota_types::digests::ChainIdentifier> {
        self.state_reader.get_chain_identifier().map_err(Into::into)
    }

    /// Get checkpoint summary by sequence number.
    pub fn get_checkpoint_summary(
        &self,
        seq: u64,
    ) -> anyhow::Result<Option<CertifiedCheckpointSummary>> {
        self.state_reader
            .try_get_checkpoint_by_sequence_number(seq)
            .map(|opt| opt.map(CertifiedCheckpointSummary::from))
            .map_err(Into::into)
    }

    /// Get checkpoint sequence number by digest
    pub fn get_checkpoint_sequence_number_by_digest(
        &self,
        digest: &CheckpointDigest,
    ) -> anyhow::Result<Option<u64>> {
        self.state_reader
            .try_get_checkpoint_by_digest(digest)
            .map(|opt| opt.map(|c| c.sequence_number()))
            .map_err(Into::into)
    }

    /// Get the last checkpoint of a given epoch, if any
    pub fn get_epoch_last_checkpoint(
        &self,
        epoch: u64,
    ) -> anyhow::Result<Option<CertifiedCheckpointSummary>> {
        self.state_reader
            .get_epoch_last_checkpoint(epoch)
            .map(|opt| opt.map(CertifiedCheckpointSummary::from))
            .map_err(Into::into)
    }

    /// Get a single checkpoint as chunked messages stream
    pub(crate) fn get_checkpoint_data(
        &self,
        sequence_number: u64,
        checkpoint_mask: FieldMaskTree,
        transactions_mask: Option<FieldMaskTree>,
        events_mask: Option<FieldMaskTree>,
        max_message_size_bytes: u32,
        transaction_filter: Option<crate::transaction_filter::TransactionFilter>,
        event_filter: Option<crate::event_filter::EventFilter>,
        metrics: RequestMetrics,
    ) -> std::pin::Pin<Box<dyn futures::Stream<Item = CheckpointStreamResult> + Send>> {
        let state_reader = self.state_reader.clone();
        match checkpoint_summary_and_contents(&*state_reader, sequence_number) {
            Ok(Some((checkpoint_summary, checkpoint_contents))) => {
                Box::pin(async_stream::stream! {
                    let transaction_stream = state_reader.stream_checkpoint_transactions(checkpoint_contents.clone());
                    let mut checkpoint_stream = Box::pin(Self::create_checkpoint_messages_stream(
                        checkpoint_summary,
                        checkpoint_contents,
                        transaction_stream,
                        &checkpoint_mask,
                        transactions_mask,
                        events_mask,
                        max_message_size_bytes as usize,
                        transaction_filter,
                        event_filter,
                        metrics,
                    ));

                    while let Some(result) = checkpoint_stream.next().await {
                        yield result;
                    }
                })
            }
            Ok(None) => Box::pin(async_stream::stream! {
                yield Err(Status::not_found(format!(
                    "Checkpoint {sequence_number} not found"
                )));
            }),
            Err(e) => Box::pin(async_stream::stream! {
                yield Err(Status::internal(format!(
                    "Failed to get checkpoint {sequence_number}: {e}"
                )));
            }),
        }
    }

    /// Helper function to create checkpoint messages from checkpoint data as a
    /// stream. Sends Checkpoint first (based on checkpoint_mask), then
    /// Transactions (batched by size), then Events (if requested), and
    /// finally EndMarker.
    ///
    /// Generic over transaction stream source - works for both historical and
    /// live data.
    fn create_checkpoint_messages_stream<S>(
        checkpoint_summary: CertifiedCheckpointSummary,
        checkpoint_contents: CheckpointContents,
        transaction_stream: S,
        checkpoint_mask: &FieldMaskTree,
        transactions_mask: Option<FieldMaskTree>,
        events_mask: Option<FieldMaskTree>,
        max_message_size_bytes: usize,
        transaction_filter: Option<crate::transaction_filter::TransactionFilter>,
        event_filter: Option<crate::event_filter::EventFilter>,
        metrics: RequestMetrics,
    ) -> impl futures::Stream<Item = Result<grpc_ledger_service::CheckpointData, Status>> + Send
    where
        S: futures::Stream<Item = anyhow::Result<IotaTypesCheckpointTransaction>> + Send,
    {
        use grpc_ledger_service::checkpoint_data::EndMarker;

        // Clone values needed across the async boundary
        let checkpoint_mask = checkpoint_mask.clone();

        async_stream::stream! {
            let sequence_number = checkpoint_summary.data().sequence_number;
            let mut checkpoint_size_tracker = CheckpointSizeTracker::new(&metrics);

            // 1. Send Checkpoint message (controlled by checkpoint_mask)
            // Build the Checkpoint proto message using Merge

            // We need the sequence number to reassemble the checkpoint on client side.
            let mut checkpoint_proto = grpc_checkpoint::Checkpoint::default()
                .with_sequence_number(sequence_number);

            // Use Merge to populate based on mask
            Merge::merge(&mut checkpoint_proto, checkpoint_summary.data(), &checkpoint_mask)
                .map_err(|e| e.with_context("failed to merge summary"))?;
            Merge::merge(&mut checkpoint_proto, &checkpoint_contents, &checkpoint_mask)
                .map_err(|e| e.with_context("failed to merge contents"))?;
            Merge::merge(&mut checkpoint_proto, checkpoint_summary.auth_sig(), &checkpoint_mask)
                .map_err(|e| e.with_context("failed to merge signature"))?;

            let checkpoint_message = grpc_ledger_service::CheckpointData::default().with_checkpoint(checkpoint_proto);
            checkpoint_size_tracker.add_message_size(&checkpoint_message);
            yield Ok(checkpoint_message);

            // 2. Stream transactions and events if requested (interleaved)
            if transactions_mask.is_some() || events_mask.is_some() {
                let tx_mask = transactions_mask.clone().unwrap_or_else(FieldMaskTree::new_wildcard);
                let should_collect_events = events_mask.is_some();
                let events_submask = events_mask
                    .as_ref()
                    .and_then(|m| m.subtree("events"))
                    .unwrap_or_else(FieldMaskTree::new_wildcard);

                let mut transaction_stream = Box::pin(transaction_stream);
                let mut current_batch: Vec<grpc_transaction::ExecutedTransaction> = Vec::new();
                let mut current_batch_size = 0usize;

                // Event batching state
                let mut events_batch: Vec<grpc_event::Event> = Vec::new();
                let mut events_batch_size = 0usize;

                while let Some(result) = transaction_stream.next().await {
                    match result {
                        Ok(checkpoint_transaction) => {
                            // Everything still alive at a yield stays in memory until
                            // the client reads that message. Build the event and
                            // transaction protos first, which consumes the raw
                            // transaction, and yield batches only after that.
                            let mut new_events: Vec<grpc_event::Event> = Vec::new();
                            if should_collect_events {
                                if let Some(tx_events) = &checkpoint_transaction.events {
                                    // Filter raw events before SDK conversion
                                    for raw_event in &tx_events.0 {
                                        // Apply event filter if present
                                        if let Some(ref evt_filter) = event_filter {
                                            if !evt_filter.matches_event(raw_event) {
                                                continue; // Skip non-matching events
                                            }
                                        }
                                        new_events.push(
                                            grpc_event::Event::merge_from(raw_event, &events_submask)
                                                .map_err(|e| e.with_context("failed to merge event"))?,
                                        );
                                    }
                                }
                            }

                            // Build the transaction proto only if transactions_mask is
                            // requested and it passes the transaction filter.
                            let new_executed_tx = if transactions_mask.is_some()
                                && transaction_filter
                                    .as_ref()
                                    .is_none_or(|tx_filter| tx_filter.matches_transaction(&checkpoint_transaction))
                            {
                                let checkpoint_tx_ctx = CheckpointTransactionWithContext::new(
                                    checkpoint_transaction,
                                    Some(sequence_number),
                                    Some(checkpoint_summary.data().timestamp_ms),
                                );
                                Some(
                                    grpc_transaction::ExecutedTransaction::merge_from(
                                        checkpoint_tx_ctx,
                                        &tx_mask,
                                    )
                                    .map_err(|e| e.with_context("failed to merge transaction"))?,
                                )
                            } else {
                                drop(checkpoint_transaction);
                                None
                            };

                            // Check the transaction's own size right after building it,
                            // before it can be held alive across any event-batch yield
                            // below.
                            let new_executed_tx = match new_executed_tx {
                                Some(executed_tx) => {
                                    let tx_encoded_len = executed_tx.encoded_len();
                                    metrics.record_response_item(ResponseItemKind::Transaction, tx_encoded_len);
                                    let tx_size = tx_encoded_len + crate::utils::repeated_field_item_overhead(tx_encoded_len);
                                    let tx_total = tx_size + crate::utils::checkpoint_data_wrapper_overhead(tx_size);
                                    if tx_total > max_message_size_bytes {
                                        yield Err(Status::invalid_argument(format!(
                                            "Single transaction size ({tx_total} bytes) exceeds max message size ({max_message_size_bytes} bytes)"
                                        )));
                                        return;
                                    }
                                    Some((executed_tx, tx_size))
                                }
                                None => None,
                            };

                            // Collect and yield events as they reach size limits
                            for grpc_event in new_events {
                                let event_encoded_len = grpc_event.encoded_len();
                                let event_size = event_encoded_len + crate::utils::repeated_field_item_overhead(event_encoded_len);
                                metrics.record_response_item(ResponseItemKind::Event, event_encoded_len);

                                // Check if a single event exceeds the message size limit
                                let event_total = event_size + crate::utils::checkpoint_data_wrapper_overhead(event_size);
                                if event_total > max_message_size_bytes {
                                    yield Err(Status::invalid_argument(format!(
                                        "Single event size ({event_total} bytes) exceeds max message size ({max_message_size_bytes} bytes)"
                                    )));
                                    return;
                                }

                                // Check if adding this event would exceed limit
                                // (batch content + wrapper overhead for CheckpointData oneof)
                                let candidate_size = events_batch_size + event_size;
                                if candidate_size + crate::utils::checkpoint_data_wrapper_overhead(candidate_size) > max_message_size_bytes && !events_batch.is_empty() {
                                    // Yield current event batch
                                    checkpoint_size_tracker.add_batch_size(events_batch_size);
                                    yield Ok(grpc_ledger_service::CheckpointData::default()
                                        .with_events(grpc_event::Events::default().with_events(events_batch)));

                                    // Reset event batch
                                    events_batch = vec![grpc_event];
                                    events_batch_size = event_size;
                                } else {
                                    events_batch.push(grpc_event);
                                    events_batch_size += event_size;
                                }
                            }

                            if let Some((executed_tx, tx_size)) = new_executed_tx {
                                // Check if adding this tx would exceed limit
                                // (batch content + wrapper overhead for CheckpointData oneof)
                                let candidate_size = current_batch_size + tx_size;
                                if candidate_size + crate::utils::checkpoint_data_wrapper_overhead(candidate_size) > max_message_size_bytes && !current_batch.is_empty() {
                                    // Yield current transaction batch
                                    checkpoint_size_tracker.add_batch_size(current_batch_size);
                                    yield Ok(grpc_ledger_service::CheckpointData::default()
                                        .with_executed_transactions(grpc_transaction::ExecutedTransactions::default().with_executed_transactions(current_batch)));

                                    // Reset transaction batch
                                    current_batch = vec![executed_tx];
                                    current_batch_size = tx_size;
                                } else {
                                    current_batch.push(executed_tx);
                                    current_batch_size += tx_size;
                                }
                            }
                        }
                        Err(e) => {
                            yield Err(Status::internal(format!("transaction stream error: {e}")));
                            return;
                        }
                    }
                }

                // Send final batch of transactions if any
                if transactions_mask.is_some() && !current_batch.is_empty() {
                    checkpoint_size_tracker.add_batch_size(current_batch_size);
                    yield Ok(grpc_ledger_service::CheckpointData::default()
                        .with_executed_transactions(grpc_transaction::ExecutedTransactions::default().with_executed_transactions(current_batch)));
                }

                // Send final batch of events if any
                if should_collect_events && !events_batch.is_empty() {
                    checkpoint_size_tracker.add_batch_size(events_batch_size);
                    yield Ok(grpc_ledger_service::CheckpointData::default()
                        .with_events(grpc_event::Events::default().with_events(events_batch)));
                }
            }

            // 3. Always send EndMarker at the end
            let end_marker = grpc_ledger_service::CheckpointData::default().with_end_marker(EndMarker::default().with_sequence_number(sequence_number));
            checkpoint_size_tracker.add_message_size(&end_marker);
            checkpoint_size_tracker.record();
            yield Ok(end_marker);
        }
    }

    /// Get the latest checkpoint sequence number
    pub fn get_latest_checkpoint_sequence_number(&self) -> anyhow::Result<Option<u64>> {
        latest_checkpoint_seq(&*self.state_reader)
    }

    pub fn get_latest_checkpoint(&self) -> anyhow::Result<CertifiedCheckpointSummary> {
        match self.state_reader.try_get_latest_checkpoint() {
            Ok(checkpoint) => Ok(CertifiedCheckpointSummary::from(checkpoint)),
            Err(e) => match e.kind() {
                Kind::Missing => Err(anyhow::anyhow!(
                    "Unable to determine current epoch: no checkpoints available"
                )),
                _ => Err(anyhow::anyhow!(
                    "Storage error getting latest checkpoint: {e}"
                )),
            },
        }
    }

    pub fn get_lowest_available_checkpoint(&self) -> anyhow::Result<u64> {
        self.state_reader
            .try_get_lowest_available_checkpoint()
            .map_err(Into::into)
    }

    pub fn get_lowest_available_checkpoint_objects(&self) -> anyhow::Result<u64> {
        self.state_reader
            .get_lowest_available_checkpoint_objects()
            .map_err(Into::into)
    }

    pub fn get_object(&self, object_id: &ObjectId) -> anyhow::Result<Option<Object>> {
        self.state_reader
            .try_get_object(object_id)
            .map_err(Into::into)
    }

    pub fn get_object_by_key(
        &self,
        object_id: &ObjectId,
        version: VersionNumber,
    ) -> anyhow::Result<Option<Object>> {
        self.state_reader
            .try_get_object_by_key(object_id, version)
            .map_err(Into::into)
    }

    pub fn get_committee(
        &self,
        epoch: u64,
    ) -> anyhow::Result<Option<Arc<iota_types::committee::Committee>>> {
        self.state_reader
            .try_get_committee(epoch)
            .map_err(Into::into)
    }

    pub fn get_system_state(
        &self,
    ) -> anyhow::Result<iota_types::iota_system_state::IotaSystemState> {
        iota_types::iota_system_state::get_iota_system_state(self.state_reader.as_ref())
            .map_err(Into::into)
    }

    pub fn get_system_state_summary(
        &self,
    ) -> anyhow::Result<
        iota_types::iota_system_state::iota_system_state_summary::IotaSystemStateSummary,
    > {
        use iota_types::iota_system_state::IotaSystemStateTrait;

        let system_state = self.get_system_state()?;
        let summary = system_state.into_iota_system_state_summary();

        Ok(summary)
    }

    pub fn get_epoch_info(
        &self,
        epoch: u64,
    ) -> anyhow::Result<Option<iota_types::storage::EpochInfoV2>> {
        self.state_reader.get_epoch_info(epoch).map_err(Into::into)
    }

    pub fn get_type_layout(
        &self,
        type_tag: &TypeTag,
    ) -> anyhow::Result<Option<move_core_types::annotated_value::MoveTypeLayout>> {
        self.state_reader
            .get_type_layout(type_tag)
            .map_err(Into::into)
    }

    /// Iterate over objects owned by an account address.
    ///
    /// The cursor is exclusive: items *after* the cursor position are returned.
    pub fn account_owned_objects_info_iter(
        &self,
        owner: Address,
        cursor: Option<&OwnedObjectCursor>,
        object_type: Option<StructTag>,
    ) -> Result<Box<dyn Iterator<Item = OwnedObjectIterItem> + '_>, crate::error::RpcError> {
        let indexes = self.require_indexes()?;
        let iter = indexes
            .account_owned_objects_info_iter(owner, cursor, object_type)
            .map_err(|e| crate::error::RpcError::internal().with_context(e))?;
        Ok(Box::new(iter.map(|r| r.map_err(Into::into))))
    }

    /// Iterate over dynamic fields of a parent object.
    ///
    /// The cursor is exclusive: items *after* the cursor position are
    /// returned.
    pub fn dynamic_field_iter(
        &self,
        parent: ObjectId,
        cursor: Option<ObjectId>,
    ) -> anyhow::Result<Box<dyn Iterator<Item = DynamicFieldIterItem> + '_>> {
        let iter = self.require_indexes()?.dynamic_field_iter(parent, cursor)?;
        Ok(Box::new(iter.map(|r| r.map_err(Into::into))))
    }

    /// Get unified coin info.
    pub fn get_coin_info(
        &self,
        coin_type: &StructTag,
    ) -> Result<Option<iota_types::storage::CoinInfo>, crate::error::RpcError> {
        let indexes = self.require_indexes()?;
        let info = indexes
            .get_coin_info(coin_type)
            .map_err(|e| crate::error::RpcError::internal().with_context(e))?;
        Ok(info)
    }

    /// Iterate over all versions of a package by its original package ID.
    ///
    /// The cursor is exclusive: items *after* the cursor position are
    /// returned.
    pub fn package_versions_iter(
        &self,
        original_package_id: ObjectId,
        cursor: Option<u64>,
    ) -> Result<Box<dyn Iterator<Item = PackageVersionIterItem> + '_>, crate::error::RpcError> {
        let indexes = self.require_indexes()?;
        let iter = indexes
            .package_versions_iter(original_package_id, cursor)
            .map_err(|e| crate::error::RpcError::internal().with_context(e))?;
        Ok(Box::new(iter.map(|r| r.map_err(Into::into))))
    }

    /// Generic stream implementation for checkpoints
    fn create_generic_checkpoint_stream<T, S, R>(
        &self,
        mut rx: Receiver<Arc<T>>,
        subscriber_guard: SubscriberGuard,
        start_sequence_number: Option<u64>,
        end_sequence_number: Option<u64>,
        cancellation_token: CancellationToken,
        data_type_name: &'static str,
        fetch_historical: impl Fn(
            Arc<dyn iota_node_storage::GrpcStateReader>,
            u64,
        ) -> Result<Option<Arc<S>>, Status>
        + Send,
        get_sequence_number_live: impl Fn(&Arc<T>) -> u64 + Send,
        process_item_historical: impl Fn(
            Arc<S>,
        ) -> std::pin::Pin<
            Box<dyn futures::Stream<Item = Result<R, Status>> + Send>,
        > + Send,
        process_item_live: impl Fn(
            Arc<T>,
        ) -> std::pin::Pin<
            Box<dyn futures::Stream<Item = Result<R, Status>> + Send>,
        > + Send,
    ) -> impl futures::Stream<Item = Result<R, Status>> + Send
    where
        T: Send + Sync + 'static,
        S: Send + Sync + 'static,
        R: Send + 'static,
    {
        let state_reader = self.state_reader.clone();
        async_stream::try_stream! {
            // Capture the guard so it drops (releasing the subscriber slot
            // and decrementing the gauge) when the stream is dropped.
            let _subscriber_guard = subscriber_guard;
            let mut latest = latest_checkpoint_seq(&*state_reader)
                .map_err(|e| Status::internal(format!("Failed to get latest checkpoint: {e}")))?
                .unwrap_or(0);
            debug!("[profile][grpc] Latest checkpoint index: {latest}.");
            let (mut start, end) = match (start_sequence_number, end_sequence_number) {
                (None, None) => (latest, u64::MAX),
                (None, Some(end)) => (end, end),
                (Some(start), None) => (start, u64::MAX),
                (Some(start), Some(end)) => (start, end),
            };

            while start <= end {
                // Try fetching historical data from the DB first
                if start <= latest {
                    // Check if the checkpoint has been pruned since we started
                    // (e.g. genesis checkpoint 0 is always in DB but may be
                    // below the pruning watermark).
                    let lowest_available = state_reader
                        .try_get_lowest_available_checkpoint()
                        .map_err(|e| Status::internal(format!("Failed to get lowest available checkpoint: {e}")))?;
                    if start < lowest_available {
                        Err(Status::not_found(format!(
                            "Checkpoint {data_type_name} {start} is below the lowest available checkpoint {lowest_available}"
                        )))?;
                    }

                    match fetch_historical(state_reader.clone(), start)? {
                        Some(item) => {
                            debug!("[profile][grpc] Fetched checkpoint {data_type_name} for index {start} from DB.");

                            // Process the item and yield all results
                            let mut item_stream = process_item_historical(item);
                            while let Some(result) = item_stream.next().await {
                                yield result?;
                            }

                            if start == end {
                                break;
                            }
                            start += 1;
                            continue;
                        }
                        None => {
                            Err(Status::not_found(format!("Historical checkpoint {data_type_name} missing/pruned: index={start} latest={latest}.")))?;
                        }
                    }
                }

                // Live phase - wait for broadcast or cancellation
                let item_result = tokio::select! {
                    recv_result = rx.recv() => Some(recv_result),
                    _ = cancellation_token.cancelled() => {
                        debug!("[profile][grpc] Checkpoint {data_type_name} stream cancelled");
                        None
                    }
                };

                match item_result {
                    Some(Ok(item)) => {
                        debug!("[profile][grpc] Get checkpoint {data_type_name} for index {} from broadcast channel", get_sequence_number_live(&item));
                        let sequence_number = get_sequence_number_live(&item);
                        if start == sequence_number {
                            // Process the item and yield all results
                            let mut item_stream = process_item_live(item);
                            while let Some(result) = item_stream.next().await {
                                yield result?;
                            }

                            if start == end {
                                break;
                            }
                            start += 1;
                            continue;
                        }
                        // else item sequence doesn't match, drop it and continue
                    }
                    Some(Err(RecvError::Lagged(_))) => {
                        // continue, lagged item should be picked up from history DB
                    }
                    Some(Err(RecvError::Closed)) => {
                        Err(Status::internal(format!("Checkpoint {data_type_name} channel closed.")))?;
                        break;
                    }
                    None => {
                        // Cancellation was triggered
                        break;
                    }
                }
                latest = latest_checkpoint_seq(&*state_reader)
                    .map_err(|e| Status::internal(format!("Failed to get latest checkpoint: {e}")))?
                    .unwrap_or(start);
                debug!("[profile][grpc] Updating latest checkpoint index to {latest}.");
            }
        }
    }

    /// Lightweight check to determine if a checkpoint has any matching data
    /// without performing full SDK conversion or Merge operations.
    /// Returns true on first match (OR semantics when both filters are set).
    async fn has_matching_data<S>(
        transaction_stream: S,
        transaction_filter: &Option<crate::transaction_filter::TransactionFilter>,
        event_filter: &Option<crate::event_filter::EventFilter>,
    ) -> Result<bool, Status>
    where
        S: futures::Stream<Item = anyhow::Result<IotaTypesCheckpointTransaction>> + Send,
    {
        let mut transaction_stream = std::pin::pin!(transaction_stream);
        while let Some(result) = transaction_stream.next().await {
            let checkpoint_transaction =
                result.map_err(|e| Status::internal(format!("failed to read transaction: {e}")))?;

            if let Some(ref tx_filter) = transaction_filter {
                if tx_filter.matches_transaction(&checkpoint_transaction) {
                    return Ok(true);
                }
            }

            if let Some(ref evt_filter) = event_filter {
                if let Some(ref tx_events) = checkpoint_transaction.events {
                    for event in &tx_events.0 {
                        if evt_filter.matches_event(event) {
                            return Ok(true);
                        }
                    }
                }
            }
        }
        Ok(false)
    }

    /// Tests whether any transaction in a checkpoint matches the active
    /// filters (transaction and/or event).
    ///
    /// Returns [`FilterCheckResult::Matched`] if at least one transaction
    /// matches, signalling that the checkpoint should be fully processed.
    /// Returns [`FilterCheckResult::Skipped`] otherwise, attaching a
    /// progress heartbeat when `progress_interval` has elapsed since the
    /// last emitted message.
    async fn match_checkpoint_filter_or_report_progress<S>(
        transaction_stream: S,
        transaction_filter: &Option<crate::transaction_filter::TransactionFilter>,
        event_filter: &Option<crate::event_filter::EventFilter>,
        last_msg_time: &std::sync::Mutex<tokio::time::Instant>,
        progress_interval: std::time::Duration,
        seq: u64,
    ) -> Result<FilterCheckResult, Status>
    where
        S: futures::Stream<Item = anyhow::Result<IotaTypesCheckpointTransaction>> + Send,
    {
        if Self::has_matching_data(transaction_stream, transaction_filter, event_filter).await? {
            *last_msg_time.lock().unwrap() = tokio::time::Instant::now();
            Ok(FilterCheckResult::Matched)
        } else {
            let progress = {
                let mut guard = last_msg_time.lock().unwrap();
                if guard.elapsed() >= progress_interval {
                    *guard = tokio::time::Instant::now();
                    Some(
                        grpc_ledger_service::CheckpointData::default().with_progress(
                            Progress::default().with_latest_scanned_sequence_number(seq),
                        ),
                    )
                } else {
                    None
                }
            };
            Ok(FilterCheckResult::Skipped(progress))
        }
    }

    /// Create a checkpoint stream implementation
    pub(crate) fn create_checkpoint_data_stream(
        &self,
        subscription: SubscribedReceiver,
        start_sequence_number: Option<u64>,
        end_sequence_number: Option<u64>,
        checkpoint_mask: FieldMaskTree,
        transactions_mask: Option<FieldMaskTree>,
        events_mask: Option<FieldMaskTree>,
        max_message_size_bytes: u32,
        cancellation_token: CancellationToken,
        transaction_filter: Option<crate::transaction_filter::TransactionFilter>,
        event_filter: Option<crate::event_filter::EventFilter>,
        filter_checkpoints: bool,
        progress_interval: std::time::Duration,
        metrics: RequestMetrics,
    ) -> Box<dyn futures::Stream<Item = CheckpointStreamResult> + Send + Unpin> {
        let reader = self.clone();
        let state_reader_clone = self.state_reader.clone();

        // Shared timer for progress messages (used only when filter_checkpoints is
        // true)
        let last_message_time = Arc::new(std::sync::Mutex::new(tokio::time::Instant::now()));

        // Clone for closures
        let checkpoint_mask_historical = checkpoint_mask.clone();
        let transactions_mask_historical = transactions_mask.clone();
        let events_mask_historical = events_mask.clone();
        let transaction_filter_historical = transaction_filter.clone();
        let event_filter_historical = event_filter.clone();

        Box::new(Box::pin(reader.create_generic_checkpoint_stream(
            subscription.rx,
            subscription.guard,
            start_sequence_number,
            end_sequence_number,
            cancellation_token,
            "data",
            // Historical data fetcher - returns (summary, contents)
            |reader, seq| {
                checkpoint_summary_and_contents(&*reader, seq)
                    .map(|opt| opt.map(Arc::new))
                    .map_err(|e| {
                        Status::internal(format!("Failed to get checkpoint {seq}: {e}"))
                    })
            },
            |item| item.checkpoint_summary.sequence_number(),
            // Historical data processor - uses transaction stream from DB
            {
                let state_reader_historical = state_reader_clone.clone();
                let last_message_time_historical = last_message_time.clone();
                let metrics = metrics.clone();
                move |item: Arc<(CertifiedCheckpointSummary, CheckpointContents)>| {
                    let metrics = metrics.clone();
                    let state_reader_inner = state_reader_historical.clone();
                    let checkpoint_summary = item.0.clone();
                    let checkpoint_contents = item.1.clone();
                    let cp_mask = checkpoint_mask_historical.clone();
                    let tx_mask = transactions_mask_historical.clone();
                    let ev_mask = events_mask_historical.clone();
                    let tx_filter = transaction_filter_historical.clone();
                    let ev_filter = event_filter_historical.clone();
                    let last_msg_time = last_message_time_historical.clone();
                    {
                        Box::pin(async_stream::stream! {
                            let seq = checkpoint_summary.data().sequence_number;

                            // Pass 1: lightweight filter check when filter_checkpoints is enabled
                            if filter_checkpoints {
                                let scan_stream = state_reader_inner.stream_checkpoint_transactions(checkpoint_contents.clone());
                                match Self::match_checkpoint_filter_or_report_progress(
                                    scan_stream,
                                    &tx_filter,
                                    &ev_filter,
                                    &last_msg_time,
                                    progress_interval,
                                    seq,
                                ).await? {
                                    FilterCheckResult::Matched => {}
                                    FilterCheckResult::Skipped(progress) => {
                                        if let Some(msg) = progress {
                                            yield Ok(msg);
                                        }

                                        // no filter match, skip processing this checkpoint
                                        return;
                                    }
                                }
                            }

                            // Pass 2 (or normal mode): full processing
                            let transaction_stream = state_reader_inner.stream_checkpoint_transactions(checkpoint_contents.clone());
                            let mut stream = Box::pin(Self::create_checkpoint_messages_stream(
                                checkpoint_summary,
                                checkpoint_contents,
                                transaction_stream,
                                &cp_mask,
                                tx_mask,
                                ev_mask,
                                max_message_size_bytes as usize,
                                tx_filter,
                                ev_filter,
                                metrics,
                            ));

                            while let Some(item) = stream.next().await {
                                yield item;
                            }
                        })
                    }
                }
            },
            // Live data processor - extracts transactions from CheckpointData
            {
                let last_message_time_live = last_message_time;
                move |item: Arc<IotaTypesCheckpointData>| {
                    let metrics = metrics.clone();
                    let cp_mask = checkpoint_mask.clone();
                    let tx_mask = transactions_mask.clone();
                    let ev_mask = events_mask.clone();
                    let tx_filter = transaction_filter.clone();
                    let ev_filter = event_filter.clone();
                    let last_msg_time = last_message_time_live.clone();
                    Box::pin(async_stream::stream! {
                        let seq = item.checkpoint_summary.sequence_number();

                        // Pass 1: lightweight filter check when filter_checkpoints is enabled
                        if filter_checkpoints {
                            let scan_stream = live_checkpoint_transaction_stream(item.clone());
                            match Self::match_checkpoint_filter_or_report_progress(
                                scan_stream,
                                &tx_filter,
                                &ev_filter,
                                &last_msg_time,
                                progress_interval,
                                seq,
                            ).await? {
                                FilterCheckResult::Matched => {}
                                FilterCheckResult::Skipped(progress) => {
                                    if let Some(msg) = progress {
                                        yield Ok(msg);
                                    }

                                    // no filter match, skip processing this checkpoint
                                    return;
                                }
                            }
                        }

                        // Pass 2 (or normal mode): full processing
                        let transaction_stream = live_checkpoint_transaction_stream(item.clone());

                        // Use the unified streaming function
                        let mut stream = Box::pin(Self::create_checkpoint_messages_stream(
                            item.checkpoint_summary.clone(),
                            item.checkpoint_contents.clone(),
                            transaction_stream,
                            &cp_mask,
                            tx_mask,
                            ev_mask,
                            max_message_size_bytes as usize,
                            tx_filter,
                            ev_filter,
                            metrics,
                        ));

                        while let Some(item) = stream.next().await {
                            yield item;
                        }
                    })
                }
            },
        )))
    }

    /// Get transaction data for a single transaction digest.
    ///
    /// Only fetches data from storage when indicated by `fields`, enabling
    /// callers to skip unnecessary reads. Effects are fetched when any of
    /// effects/events/input_objects/output_objects are requested since they
    /// provide the digests and references needed to fetch those fields.
    /// Balance/object changes are derived fields: they additionally force the
    /// fetch of effects and input/output objects, and object changes force the
    /// transaction fetch (for the sender). Over-fetched data never leaks into
    /// the response — the `Merge` impls only populate mask-requested fields.
    ///
    /// Errors with `FAILED_PRECONDITION` if a required object is unavailable
    /// (e.g. pruned): a silently incomplete object set would be undetectable
    /// by the client and would corrupt derived change fields.
    #[tracing::instrument(skip(self))]
    pub fn get_transaction_read(
        &self,
        digest: &TransactionDigest,
        fields: &TransactionReadFields,
    ) -> Result<TransactionReadData, crate::error::RpcError> {
        let (transaction, signatures) = if fields.include_transaction
            || fields.include_signatures
            || fields.include_object_changes
        {
            // Get the transaction if transaction data or signatures are requested
            let transaction = self
                .state_reader
                .try_get_transaction(digest)?
                .ok_or(crate::error::TransactionNotFoundError(*digest))?;

            let transaction_data = (fields.include_transaction || fields.include_object_changes)
                .then(|| transaction.transaction().clone());

            let signatures_data = fields
                .include_signatures
                .then(|| transaction.signatures().to_owned());

            (transaction_data, signatures_data)
        } else {
            (None, None)
        };

        let (checkpoint, timestamp_ms) = if fields.include_checkpoint || fields.include_timestamp {
            let checkpoint = self
                .require_indexes()?
                .get_transaction_info(digest)?
                .map(|info| info.checkpoint);

            let timestamp_ms = if fields.include_timestamp {
                match checkpoint {
                    Some(checkpoint_seq) => {
                        let summary = self
                            .state_reader
                            .try_get_checkpoint_by_sequence_number(checkpoint_seq)?
                            .ok_or_else(|| {
                                crate::error::RpcError::new(
                                    tonic::Code::Internal,
                                    format!(
                                        "Checkpoint summary {checkpoint_seq} not found for transaction {digest}"
                                    ),
                                )
                            })?;
                        Some(summary.data().timestamp_ms)
                    }
                    // Transaction not yet included in a checkpoint
                    None => None,
                }
            } else {
                None
            };
            (checkpoint, timestamp_ms)
        } else {
            (None, None)
        };

        // Derived change fields need effects plus the input/output objects
        let include_derived_changes =
            fields.include_balance_changes || fields.include_object_changes;

        // Get the effects if any of the following are requested: effects, events,
        // checkpoint/timestamp, input/output objects, balance/object changes
        let (effects, events, input_objects, output_objects) = if fields.include_effects
            || fields.include_events
            || fields.include_input_objects
            || fields.include_output_objects
            || include_derived_changes
        {
            // Effects are required for events and input/output objects, so we fetch them if
            // any of those are requested
            let effects = self
                .state_reader
                .try_get_transaction_effects(digest)?
                .ok_or(crate::error::TransactionNotFoundError(*digest))?;

            // Get events only if requested
            let events = if fields.include_events {
                match effects.events_digest() {
                    Some(_) => self.state_reader.try_get_events(digest)?,
                    None => None,
                }
            } else {
                None
            };

            // The object sets must be complete: a silently missing object
            // would shorten the input/output object lists and corrupt any
            // derived change fields, with no way for the client to detect it
            let require_object = |object_id: &iota_sdk_types::ObjectId,
                                  version: Version|
             -> Result<Object, crate::error::RpcError> {
                self.state_reader
                    .try_get_object_by_key(object_id, version)?
                    .ok_or_else(|| {
                        crate::error::RpcError::new(
                            tonic::Code::FailedPrecondition,
                            format!(
                                "object {object_id} at version {version} required by the \
                                 requested fields is unavailable (possibly pruned); narrow the \
                                 read_mask or fetch objects individually via `get_objects` for best-effort retrieval"
                            ),
                        )
                    })
            };

            // Get input objects only if requested
            let input_objects = if fields.include_input_objects || include_derived_changes {
                let mut objects = Vec::new();
                for modified in effects.modified_at_versions() {
                    objects.push(require_object(modified.object_id(), modified.version())?);
                }
                Some(objects)
            } else {
                None
            };

            // Get output objects only if requested
            let output_objects = if fields.include_output_objects || include_derived_changes {
                let mut objects = Vec::new();
                for written in effects
                    .created()
                    .into_iter()
                    .chain(effects.mutated())
                    .chain(effects.unwrapped())
                {
                    let object_ref = written.reference();
                    objects.push(require_object(&object_ref.object_id, object_ref.version)?);
                }
                Some(objects)
            } else {
                None
            };

            (Some(effects), events, input_objects, output_objects)
        } else {
            // If none of the above are requested, we can skip fetching effects entirely
            (None, None, None, None)
        };

        Ok(TransactionReadData {
            digest: *digest,
            transaction,
            signatures,
            effects,
            events,
            checkpoint,
            timestamp_ms,
            input_objects,
            output_objects,
        })
    }
}

/// Internal struct to hold all transaction-related data fetched from storage.
///
/// This struct holds owned data from storage, which is then converted to
/// `iota-sdk-types` types and used with `Merge` trait to populate gRPC
/// responses.
///
/// Optional fields are `None` when the corresponding data was not requested
/// via `TransactionReadFields`, meaning the storage read was skipped entirely.
#[derive(Debug)]
pub struct TransactionReadData {
    pub digest: TransactionDigest,
    pub transaction: Option<iota_sdk_types::Transaction>,
    pub signatures: Option<Vec<iota_sdk_types::UserSignature>>,
    pub effects: Option<TransactionEffects>,
    pub events: Option<TransactionEvents>,
    pub checkpoint: Option<u64>,
    pub timestamp_ms: Option<u64>,
    pub input_objects: Option<Vec<Object>>,
    pub output_objects: Option<Vec<Object>>,
}

/// Wrapper type that includes checkpoint context for a CheckpointTransaction.
#[derive(Debug, Clone)]
pub struct CheckpointTransactionWithContext {
    pub transaction: iota_types::full_checkpoint_content::CheckpointTransaction,
    pub checkpoint_sequence_number: Option<u64>,
    pub checkpoint_timestamp_ms: Option<u64>,
}

impl CheckpointTransactionWithContext {
    pub fn new(
        transaction: iota_types::full_checkpoint_content::CheckpointTransaction,
        checkpoint_sequence_number: Option<u64>,
        checkpoint_timestamp_ms: Option<u64>,
    ) -> Self {
        Self {
            transaction,
            checkpoint_sequence_number,
            checkpoint_timestamp_ms,
        }
    }
}

impl Merge<CheckpointTransactionWithContext>
    for iota_grpc_types::v1::transaction::ExecutedTransaction
{
    type Error = RpcError;

    fn merge(
        &mut self,
        source: CheckpointTransactionWithContext,
        mask: &FieldMaskTree,
    ) -> Result<(), Self::Error> {
        if let Some(submask) = mask.subtree(Self::TRANSACTION_FIELD.name) {
            self.transaction = Some(iota_grpc_types::v1::transaction::Transaction::merge_from(
                &source.transaction.transaction,
                &submask,
            )?);
        }

        if let Some(submask) = mask.subtree(Self::SIGNATURES_FIELD.name) {
            self.signatures = Some(iota_grpc_types::v1::signatures::UserSignatures::merge_from(
                &source.transaction.transaction,
                &submask,
            )?);
        }

        if let Some(submask) = mask.subtree(Self::EFFECTS_FIELD.name) {
            self.effects = Some(
                iota_grpc_types::v1::transaction::TransactionEffects::merge_from(
                    &source.transaction.effects,
                    &submask,
                )?,
            );
        }

        if let Some(submask) = mask.subtree(Self::EVENTS_FIELD.name) {
            // Use unwrap_or_default so that when no events were emitted we still
            // compute a real digest (hash of the empty list) and populate an empty
            // events vec — to distinguish between "no events" and "events
            // not requested in the mask".
            self.events = Some(grpc_transaction::TransactionEvents::merge_from(
                &source.transaction.events.unwrap_or_default(),
                &submask,
            )?);
        }

        // Set checkpoint sequence number if requested
        if mask.contains(Self::CHECKPOINT_FIELD.name) {
            self.checkpoint = source.checkpoint_sequence_number;
        }

        // Set checkpoint timestamp if requested
        if mask.contains(Self::TIMESTAMP_FIELD.name) {
            self.timestamp = source.checkpoint_timestamp_ms.map(timestamp_ms_to_proto);
        }

        // Derive balance changes if requested. Checkpoint transactions always
        // carry effects and input/output objects, so no extra fetches needed.
        if mask.subtree(Self::BALANCE_CHANGES_FIELD.name).is_some() {
            self.balance_changes = Some(
                iota_grpc_types::v1::transaction::BalanceChanges::default().with_balance_changes(
                    source
                        .transaction
                        .effects
                        .as_v1()
                        .balance_changes(
                            source.transaction.input_objects.iter().map(|o| &**o),
                            source.transaction.output_objects.iter().map(|o| &**o),
                            None,
                        )?
                        .into_iter()
                        .map(crate::changes::balance_change_to_proto)
                        .collect(),
                ),
            );
        }

        // Derive object changes if requested
        if mask.subtree(Self::OBJECT_CHANGES_FIELD.name).is_some() {
            use iota_types::transaction::TransactionAPI as _;

            let sender = source.transaction.transaction.transaction().sender();
            self.object_changes = Some(
                iota_grpc_types::v1::transaction::ObjectChanges::default().with_object_changes(
                    source
                        .transaction
                        .effects
                        .as_v1()
                        .object_changes(
                            sender,
                            source.transaction.input_objects.iter().map(|o| &**o),
                            source.transaction.output_objects.iter().map(|o| &**o),
                        )?
                        .into_iter()
                        .map(crate::changes::object_change_to_proto)
                        .collect(),
                ),
            );
        }

        if let Some(submask) = mask.subtree(Self::INPUT_OBJECTS_FIELD.name) {
            self.input_objects = Some(iota_grpc_types::v1::object::Objects::merge_from(
                Some(source.transaction.input_objects),
                &submask,
            )?);
        }

        if let Some(submask) = mask.subtree(Self::OUTPUT_OBJECTS_FIELD.name) {
            self.output_objects = Some(iota_grpc_types::v1::object::Objects::merge_from(
                Some(source.transaction.output_objects),
                &submask,
            )?);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        alloc::{GlobalAlloc, Layout, System},
        sync::atomic::{AtomicUsize, Ordering},
    };

    use iota_sdk_types::{CheckpointSummary, Event, Identifier, MoveStruct, Owner};
    use iota_test_transaction_builder::TestTransactionBuilder;
    use iota_types::{
        base_types::random_object_ref,
        crypto::{AccountPrivateKey, AuthorityStrongQuorumSignInfo, get_key_pair},
        effects::TransactionEffectsExtForTesting as _,
        gas_coin::GasCoin,
        messages_checkpoint::CheckpointContentsExt as _,
        object::{MoveStructExt as _, OBJECT_START_VERSION},
        transaction::VerifiedTransaction,
    };

    use super::*;

    /// Allocations at least this big are assumed to be the test's large
    /// output-object payload: every other allocation in this test (proto
    /// buffers, small event contents, bookkeeping) is far smaller.
    const LARGE_ALLOC_THRESHOLD: usize = 512 * 1024;

    /// Bytes currently allocated in allocations of at least
    /// `LARGE_ALLOC_THRESHOLD`. This test drives the stream itself, one
    /// `.next().await` at a time, instead of through a real client and
    /// server: a real round trip gives the server task no reason to stay
    /// suspended at a `yield` once the client has read it, so it cannot
    /// tell a fixed handler from a regressed one that drops its data a
    /// little later. That also means this allocator, and the one test
    /// below that uses it, must stay in this lib's own unit tests rather
    /// than move to an integration test in `tests/`.
    static LARGE_LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

    struct CountingAllocator;

    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            if layout.size() >= LARGE_ALLOC_THRESHOLD {
                LARGE_LIVE_BYTES.fetch_add(layout.size(), Ordering::SeqCst);
            }
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            if layout.size() >= LARGE_ALLOC_THRESHOLD {
                LARGE_LIVE_BYTES.fetch_sub(layout.size(), Ordering::SeqCst);
            }
            unsafe { System.dealloc(ptr, layout) }
        }
    }

    #[global_allocator]
    static ALLOCATOR: CountingAllocator = CountingAllocator;

    /// A Move object with `padding_bytes_len` extra content bytes, big enough
    /// to be tracked by [`LARGE_ALLOC_THRESHOLD`].
    fn large_object(padding_bytes_len: usize) -> Object {
        let id = ObjectId::random();
        let mut contents = GasCoin::new(id, 100).to_bcs_bytes();
        contents.extend(vec![0u8; padding_bytes_len]);
        let move_struct = MoveStruct::new_from_execution_with_limit(
            StructTag::new_gas_coin(),
            OBJECT_START_VERSION,
            contents,
            u64::try_from(padding_bytes_len).unwrap() + 1024,
        )
        .unwrap();
        Object::new_move(
            move_struct,
            Owner::Address(Address::random()),
            TransactionDigest::GENESIS_MARKER,
        )
    }

    /// An event with `contents_len` bytes of BCS content.
    fn small_event(contents_len: usize) -> Event {
        Event {
            package_id: ObjectId::random(),
            module: Identifier::from_static("m"),
            sender: Address::random(),
            struct_tag: StructTag::new(
                Address::random(),
                Identifier::from_static("m"),
                Identifier::from_static("E"),
                vec![],
            ),
            contents: vec![0u8; contents_len],
        }
    }

    fn mock_checkpoint_summary(contents: &CheckpointContents) -> CertifiedCheckpointSummary {
        let summary = CheckpointSummary {
            epoch: 0,
            sequence_number: 0,
            network_total_transactions: 0,
            contents_digest: contents.digest(),
            previous_digest: None,
            epoch_rolling_gas_cost_summary: Default::default(),
            timestamp_ms: 0,
            checkpoint_commitments: vec![],
            end_of_epoch_data: None,
            version_specific_data: vec![],
        };
        let sig = AuthorityStrongQuorumSignInfo {
            epoch: 0,
            signature: Default::default(),
            signers_map: Default::default(),
        };
        CertifiedCheckpointSummary::new_from_data_and_sig(summary, sig)
    }

    /// The raw `CheckpointTransaction` (with its input and output objects)
    /// must not be alive at a `yield` in `create_checkpoint_messages_stream`,
    /// or a client that opens the stream and never reads it keeps that data
    /// resident.
    ///
    /// Builds one checkpoint transaction with a large (and, for this test,
    /// otherwise unused) output object and two events sized so the second
    /// event overflows the first event batch, forcing a flush `yield` while
    /// that transaction is still the "current" item. The test drives the
    /// stream by hand (one `.next().await` per message, like a stalled
    /// reader that has not asked for the next message yet) and checks, right
    /// after receiving that flush, that no large allocation is still live.
    ///
    /// Runs both with and without a `transactions_mask` (asking for
    /// transactions, but not their objects) in the same test, since the
    /// process-wide allocator above would otherwise let two such tests see
    /// each other's allocations.
    #[tokio::test]
    async fn events_batch_yield_does_not_hold_the_raw_checkpoint_transaction() {
        assert_events_batch_yield_holds_no_large_allocation(None).await;

        let transactions_mask = FieldMaskTree::from_field_mask(&prost_types::FieldMask {
            paths: vec!["transaction.digest".to_string()],
        });
        assert_events_batch_yield_holds_no_large_allocation(Some(transactions_mask)).await;
    }

    async fn assert_events_batch_yield_holds_no_large_allocation(
        transactions_mask: Option<FieldMaskTree>,
    ) {
        let events_mask = FieldMaskTree::new_wildcard();
        let events_submask = events_mask
            .subtree("events")
            .unwrap_or_else(FieldMaskTree::new_wildcard);

        // Measure the actual encoded size of one event instead of
        // hand-deriving the wire format, then size the batch limit so the
        // first event fits alone but a second, identical event does not.
        // The probe is a copy of the transaction's events, because the
        // encoded size depends on the random addresses in the event.
        let event = small_event(64);
        let probe = grpc_event::Event::merge_from(&event, &events_submask).unwrap();
        let probe_len = probe.encoded_len();
        let probe_size = probe_len + crate::utils::repeated_field_item_overhead(probe_len);
        let max_message_size_bytes =
            probe_size + crate::utils::checkpoint_data_wrapper_overhead(probe_size);

        let (sender, key): (_, AccountPrivateKey) = get_key_pair();
        let gas = random_object_ref();
        let tx = TestTransactionBuilder::new(sender, gas, 1000)
            .transfer(random_object_ref(), sender)
            .build_and_sign(&key);
        let digest = *tx.digest();
        let verified_tx = VerifiedTransaction::new_unchecked(tx);

        let checkpoint_transaction = IotaTypesCheckpointTransaction {
            transaction: verified_tx.into(),
            effects: TransactionEffects::new_empty_v1_for_testing(digest),
            events: Some(TransactionEvents(vec![event.clone(), event])),
            input_objects: Vec::new(),
            output_objects: vec![large_object(LARGE_ALLOC_THRESHOLD * 4)],
        };
        let transaction_stream = futures::stream::iter([Ok(checkpoint_transaction)]);

        let checkpoint_contents = CheckpointContents::new_with_digests_only_for_tests(Vec::new());
        let checkpoint_summary = mock_checkpoint_summary(&checkpoint_contents);

        let stream = GrpcReader::create_checkpoint_messages_stream(
            checkpoint_summary,
            checkpoint_contents,
            transaction_stream,
            &FieldMaskTree::new_wildcard(),
            transactions_mask,
            Some(events_mask),
            max_message_size_bytes,
            None,
            None,
            RequestMetrics::default(),
        );
        let mut stream = std::pin::pin!(stream);

        // 1. The checkpoint summary message: no raw transaction data involved.
        let first = stream
            .next()
            .await
            .expect("checkpoint message")
            .expect("checkpoint message should be Ok");
        assert!(
            matches!(
                first.payload,
                Some(grpc_ledger_service::checkpoint_data::Payload::Checkpoint(_))
            ),
            "expected the first message to be the checkpoint summary, got {first:?}"
        );

        // 2. The events batch flush, forced by the second event.
        let second = stream
            .next()
            .await
            .expect("events message")
            .expect("events message should be Ok");
        assert!(
            matches!(
                second.payload,
                Some(grpc_ledger_service::checkpoint_data::Payload::Events(_))
            ),
            "expected the second message to be the events batch, got {second:?}"
        );

        assert_eq!(
            LARGE_LIVE_BYTES.load(Ordering::SeqCst),
            0,
            "the raw checkpoint transaction's output object is still resident \
             at the events batch yield"
        );

        // Drain the rest of the stream.
        while stream.next().await.is_some() {}
    }
}
