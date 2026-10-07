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
use iota_node_storage::{GrpcStateReader, TransactionKeyValueStoreTrait};
use iota_sdk_types::{
    Address, CheckpointContents, CheckpointDigest, ObjectId, ObjectReference, StructTag,
    TransactionDigest, TransactionEffects, TransactionEvents, TypeTag,
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
    storage::{ObjectKey, error::Kind},
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

use crate::{error::RpcError, merge::Merge};

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

/// Central gRPC data reader wrapping a [`GrpcStateReader`].
#[derive(Clone)]
pub struct GrpcReader {
    state_reader: Arc<dyn iota_node_storage::GrpcStateReader>,
    server_version: Option<String>,
    transaction_fallback: Option<Arc<dyn TransactionKeyValueStoreTrait + Send + Sync>>,
}

impl GrpcReader {
    /// Get a reference to the gRPC indexes, returning an error if they are
    /// not available (i.e. disabled or not yet initialised).
    fn require_indexes(&self) -> anyhow::Result<&dyn iota_node_storage::GrpcIndexes> {
        self.state_reader
            .grpc_indexes()
            .ok_or_else(|| anyhow::anyhow!("gRPC indexes are disabled"))
    }

    pub fn new(
        state_reader: Arc<dyn iota_node_storage::GrpcStateReader>,
        server_version: Option<String>,
    ) -> Self {
        Self {
            state_reader,
            server_version,
            transaction_fallback: None,
        }
    }

    /// Reads data the node has pruned from `store`; `None` turns the fallback
    /// off.
    pub fn with_transaction_fallback(
        mut self,
        store: Option<Arc<dyn TransactionKeyValueStoreTrait + Send + Sync>>,
    ) -> Self {
        self.transaction_fallback = store;
        self
    }

    /// Whether data the node has pruned is read from a key-value store.
    pub(crate) fn has_transaction_fallback(&self) -> bool {
        self.transaction_fallback.is_some()
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
    pub fn get_checkpoint_data(
        &self,
        sequence_number: u64,
        checkpoint_mask: FieldMaskTree,
        transactions_mask: Option<FieldMaskTree>,
        events_mask: Option<FieldMaskTree>,
        max_message_size_bytes: u32,
        transaction_filter: Option<crate::transaction_filter::TransactionFilter>,
        event_filter: Option<crate::event_filter::EventFilter>,
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
    ) -> impl futures::Stream<Item = Result<grpc_ledger_service::CheckpointData, Status>> + Send
    where
        S: futures::Stream<Item = anyhow::Result<IotaTypesCheckpointTransaction>> + Send,
    {
        use grpc_ledger_service::checkpoint_data::EndMarker;

        // Clone values needed across the async boundary
        let checkpoint_mask = checkpoint_mask.clone();

        async_stream::stream! {
            let sequence_number = checkpoint_summary.data().sequence_number;

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

            yield Ok(grpc_ledger_service::CheckpointData::default().with_checkpoint(checkpoint_proto));

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
                            // Collect and yield events as they reach size limits
                            if should_collect_events {
                                if let Some(ref tx_events) = checkpoint_transaction.events {
                                    // Filter raw events before SDK conversion
                                    for raw_event in &tx_events.0 {
                                        // Apply event filter if present
                                        if let Some(ref evt_filter) = event_filter {
                                            if !evt_filter.matches_event(raw_event) {
                                                continue; // Skip non-matching events
                                            }
                                        }
                                        let grpc_event = grpc_event::Event::merge_from(raw_event, &events_submask)
                                            .map_err(|e| e.with_context("failed to merge event"))?;
                                        let event_encoded_len = grpc_event.encoded_len();
                                        let event_size = event_encoded_len + crate::utils::repeated_field_item_overhead(event_encoded_len);

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
                                }
                            }

                            // Build transaction only if transactions_mask is requested
                            if transactions_mask.is_some() {
                                // Apply transaction filter if present
                                if let Some(ref tx_filter) = transaction_filter {
                                    if !tx_filter.matches_transaction(&checkpoint_transaction) {
                                        continue; // Skip non-matching transactions
                                    }
                                }

                                let checkpoint_tx_ctx = CheckpointTransactionWithContext::new(
                                    checkpoint_transaction,
                                    Some(sequence_number),
                                    Some(checkpoint_summary.data().timestamp_ms),
                                );
                                let executed_tx = grpc_transaction::ExecutedTransaction::merge_from(
                                    checkpoint_tx_ctx,
                                    &tx_mask,
                                )
                                .map_err(|e| e.with_context("failed to merge transaction"))?;
                                let tx_encoded_len = executed_tx.encoded_len();
                                let tx_size = tx_encoded_len + crate::utils::repeated_field_item_overhead(tx_encoded_len);

                                // Check if a single transaction exceeds the message size limit
                                let tx_total = tx_size + crate::utils::checkpoint_data_wrapper_overhead(tx_size);
                                if tx_total > max_message_size_bytes {
                                    yield Err(Status::invalid_argument(format!(
                                        "Single transaction size ({tx_total} bytes) exceeds max message size ({max_message_size_bytes} bytes)"
                                    )));
                                    return;
                                }

                                // Check if adding this tx would exceed limit
                                // (batch content + wrapper overhead for CheckpointData oneof)
                                let candidate_size = current_batch_size + tx_size;
                                if candidate_size + crate::utils::checkpoint_data_wrapper_overhead(candidate_size) > max_message_size_bytes && !current_batch.is_empty() {
                                    // Yield current transaction batch
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
                    yield Ok(grpc_ledger_service::CheckpointData::default()
                        .with_executed_transactions(grpc_transaction::ExecutedTransactions::default().with_executed_transactions(current_batch)));
                }

                // Send final batch of events if any
                if should_collect_events && !events_batch.is_empty() {
                    yield Ok(grpc_ledger_service::CheckpointData::default()
                        .with_events(grpc_event::Events::default().with_events(events_batch)));
                }
            }

            // 3. Always send EndMarker at the end
            yield Ok(grpc_ledger_service::CheckpointData::default().with_end_marker(EndMarker::default().with_sequence_number(sequence_number)));
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
        let indexes = self
            .require_indexes()
            .map_err(|e| crate::error::RpcError::internal().with_context(e))?;
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
        let indexes = self
            .require_indexes()
            .map_err(|e| crate::error::RpcError::internal().with_context(e))?;
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
        let indexes = self
            .require_indexes()
            .map_err(|e| crate::error::RpcError::internal().with_context(e))?;
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
    pub fn create_checkpoint_data_stream(
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
                move |item: Arc<(CertifiedCheckpointSummary, CheckpointContents)>| {
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
                            // Convert the transactions Vec to a stream
                            let scan_stream = futures::stream::iter(
                                item.transactions.clone().into_iter().map(Ok)
                            );
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

                        // Convert the transactions Vec to a stream
                        let transaction_stream = futures::stream::iter(
                            item.transactions.clone().into_iter().map(Ok)
                        );

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
                        ));

                        while let Some(item) = stream.next().await {
                            yield item;
                        }
                    })
                }
            },
        )))
    }

    /// Reads the transactions of `digests` and returns one result per digest, in order.
    ///
    /// Only fetches data from storage when indicated by `fields`, enabling
    /// callers to skip unnecessary reads. Effects are fetched when any of
    /// effects/events/input_objects/output_objects are requested since they
    /// provide the digests and references needed to fetch those fields, and
    /// for checkpoint/timestamp when a key-value store is set, since they
    /// decide whether the transaction is pruned.
    /// Balance/object changes are derived fields: they additionally force the
    /// fetch of effects and input/output objects, and object changes force the
    /// transaction fetch (for the sender). Over-fetched data never leaks into
    /// the response — the `Merge` impls only populate mask-requested fields.
    ///
    /// A transaction is pruned when the node lacks its effects and its
    /// checkpoint is below the node's lowest available checkpoint. The data of
    /// a pruned transaction, and any events, objects or checkpoint summary the
    /// node lacks, are read from the key-value store set with
    /// [`Self::with_transaction_fallback`], if any.
    ///
    /// The store is read with one call for each kind of data, for all the digests that need it, so
    /// a call that fails fails every digest that needed it; a failed checkpoint lookup counts as a
    /// miss while `store_budget` lasts. The calls draw on `store_budget`, which callers share
    /// across the calls of one request.
    #[tracing::instrument(skip_all, fields(transactions = digests.len()))]
    pub async fn get_transaction_reads(
        &self,
        digests: &[TransactionDigest],
        fields: &TransactionReadFields,
        store_budget: &StoreReadBudget,
    ) -> Vec<Result<TransactionReadData, RpcError>> {
        let needs = ReadNeeds::new(fields);
        let mut reads = digests
            .iter()
            .map(|digest| self.local_read(digest, &needs))
            .collect::<Vec<_>>();

        if let Some(store) = &self.transaction_fallback {
            self.read_pruned_checkpoints(store, &mut reads, &needs, store_budget)
                .await;
            read_pruned_transactions(store, &mut reads, &needs, store_budget).await;
        }
        for read in &mut reads {
            if let Ok(pending) = read {
                if let Err(error) = pending.check_found(&needs) {
                    *read = Err(error);
                }
            }
        }
        if needs.checkpoint {
            for read in &mut reads {
                if let Ok(pending) = read {
                    if let Err(error) = self.read_checkpoint(pending) {
                        *read = Err(error);
                    }
                }
            }
        }
        if fields.include_timestamp {
            self.read_timestamps(&mut reads, store_budget).await;
        }
        if fields.include_events {
            self.read_events(&mut reads, store_budget).await;
        }
        if needs.input_objects || needs.output_objects {
            self.read_objects(&mut reads, &needs, store_budget).await;
        }

        reads
            .into_iter()
            .map(|read| read.map(|pending| pending.finish(&needs)))
            .collect()
    }

    fn local_read(
        &self,
        digest: &TransactionDigest,
        needs: &ReadNeeds,
    ) -> Result<PendingRead, RpcError> {
        let mut pending = PendingRead::new(*digest);
        if needs.transaction {
            if let Some(transaction) = self.state_reader.try_get_transaction(digest)? {
                pending.set_transaction(&transaction, &needs.fields);
            }
        }
        if needs.effects || (needs.checkpoint && self.transaction_fallback.is_some()) {
            pending.data.effects = self.state_reader.try_get_transaction_effects(digest)?;
        }
        Ok(pending)
    }

    /// Sets the checkpoint of each read whose transaction the node has pruned.
    ///
    /// Asks the store: pruning deletes the node's own transaction-to-checkpoint
    /// entries no later than the effects, so the node cannot answer for a
    /// transaction it lacks the effects of.
    async fn read_pruned_checkpoints(
        &self,
        store: &Arc<dyn TransactionKeyValueStoreTrait + Send + Sync>,
        reads: &mut [Result<PendingRead, RpcError>],
        needs: &ReadNeeds,
        store_budget: &StoreReadBudget,
    ) {
        // A transaction the node has effects for is not pruned, even when it is
        // not in a checkpoint yet.
        let indexes = pending_indexes(reads, |pending| {
            pending.data.effects.is_none()
                && (needs.effects
                    || needs.checkpoint
                    || (needs.transaction && !pending.has_transaction))
        });
        if indexes.is_empty() {
            return;
        }
        let lowest_available = match self.state_reader.try_get_lowest_available_checkpoint() {
            Ok(0) => return,
            Ok(lowest_available) => lowest_available,
            Err(error) => return fail_reads(reads, &indexes, &error.into()),
        };
        let digests = digests_at(reads, &indexes);
        // A failed lookup counts as a miss: clients polling for a new
        // transaction give up on any per-item error other than `NOT_FOUND`.
        match store_read(
            store.multi_get_transactions_perpetual_checkpoints(&digests),
            store_budget,
        )
        .await
        {
            Ok(checkpoints) => {
                for (&index, checkpoint) in indexes.iter().zip(checkpoints) {
                    if let Ok(pending) = &mut reads[index] {
                        pending.pruned =
                            checkpoint.filter(|&checkpoint| checkpoint < lowest_available);
                    }
                }
            }
            Err(error) if store_budget.is_spent() => fail_reads(reads, &indexes, &error),
            Err(_) => {}
        }
    }

    fn read_checkpoint(&self, pending: &mut PendingRead) -> Result<(), RpcError> {
        pending.data.checkpoint = match pending.pruned {
            Some(checkpoint) => Some(checkpoint),
            None => self
                .require_indexes()
                .map_err(|e| RpcError::internal().with_context(e))?
                .get_transaction_info(&pending.data.digest)?
                .map(|info| info.checkpoint),
        };
        Ok(())
    }

    async fn read_timestamps(
        &self,
        reads: &mut [Result<PendingRead, RpcError>],
        store_budget: &StoreReadBudget,
    ) {
        let mut missing = Vec::new();
        for (index, read) in reads.iter_mut().enumerate() {
            let Ok(pending) = read else { continue };
            // Transaction not yet included in a checkpoint
            let Some(checkpoint_seq) = pending.data.checkpoint else {
                continue;
            };
            match self
                .state_reader
                .try_get_checkpoint_by_sequence_number(checkpoint_seq)
            {
                Ok(Some(summary)) => pending.data.timestamp_ms = Some(summary.data().timestamp_ms),
                Ok(None) if self.transaction_fallback.is_some() => {
                    missing.push((index, checkpoint_seq))
                }
                // A node keeps the summary of every transaction it holds, so without a
                // store a miss is a broken invariant.
                Ok(None) => {
                    *read = Err(RpcError::new(
                        tonic::Code::Internal,
                        format!(
                            "Checkpoint summary {checkpoint_seq} not found for transaction {}",
                            pending.data.digest
                        ),
                    ))
                }
                Err(error) => *read = Err(error.into()),
            }
        }
        let Some(store) = self
            .transaction_fallback
            .as_ref()
            .filter(|_| !missing.is_empty())
        else {
            return;
        };

        let mut checkpoints = missing
            .iter()
            .map(|&(_, checkpoint)| checkpoint)
            .collect::<Vec<_>>();
        checkpoints.sort_unstable();
        checkpoints.dedup();
        let summaries = match store_read(
            store.multi_get_checkpoints(&checkpoints, &[], &[]),
            store_budget,
        )
        .await
        {
            Ok((summaries, _, _)) => summaries,
            Err(error) => {
                let indexes = missing.iter().map(|&(index, _)| index).collect::<Vec<_>>();
                return fail_reads(reads, &indexes, &error);
            }
        };
        let timestamps = checkpoints
            .iter()
            .zip(summaries)
            .filter_map(|(&checkpoint, summary)| Some((checkpoint, summary?.data().timestamp_ms)))
            .collect::<std::collections::HashMap<_, _>>();
        for (index, checkpoint_seq) in missing {
            let Ok(pending) = &mut reads[index] else {
                continue;
            };
            match timestamps.get(&checkpoint_seq) {
                Some(&timestamp_ms) => pending.data.timestamp_ms = Some(timestamp_ms),
                None => {
                    let digest = pending.data.digest;
                    reads[index] = Err(RpcError::new(
                        tonic::Code::FailedPrecondition,
                        format!(
                            "checkpoint summary {checkpoint_seq} of transaction {digest} required \
                             by the requested fields is unavailable {}narrow the read_mask",
                            unavailable_reason(true)
                        ),
                    ))
                }
            }
        }
    }

    /// Fails a read with `FAILED_PRECONDITION` if its events are unavailable,
    /// `UNAVAILABLE` if the store read fails or the store lacks the events of a
    /// pruned transaction, and `INTERNAL` if the store returns other events.
    async fn read_events(
        &self,
        reads: &mut [Result<PendingRead, RpcError>],
        store_budget: &StoreReadBudget,
    ) {
        let mut missing = Vec::new();
        for (index, read) in reads.iter_mut().enumerate() {
            let Ok(pending) = read else { continue };
            let Some(effects) = &pending.data.effects else {
                continue;
            };
            if effects.events_digest().is_none() {
                continue;
            }
            match self.state_reader.try_get_events(&pending.data.digest) {
                Ok(Some(events)) => pending.data.events = Some(events),
                Ok(None) if self.transaction_fallback.is_some() => missing.push(index),
                Ok(None) => *read = Err(events_unavailable(&pending.data.digest, false)),
                Err(error) => *read = Err(error.into()),
            }
        }
        let Some(store) = self
            .transaction_fallback
            .as_ref()
            .filter(|_| !missing.is_empty())
        else {
            return;
        };

        let digests = digests_at(reads, &missing);
        let fetched =
            match store_read(store.multi_get_events_by_tx_digests(&digests), store_budget).await {
                Ok(fetched) => fetched,
                Err(error) => return fail_reads(reads, &missing, &error),
            };
        let mut mismatched = Vec::new();
        // A reply shorter than the request leaves the rest missing.
        let mut fetched = fetched.into_iter();
        for &index in &missing {
            let events = fetched.next().flatten();
            let Ok(pending) = &mut reads[index] else {
                continue;
            };
            let digest = pending.data.digest;
            let expected = pending
                .data
                .effects
                .as_ref()
                .and_then(|effects| effects.events_digest().copied());
            match events {
                Some(events) if Some(events.digest()) == expected => {
                    pending.data.events = Some(events)
                }
                Some(_) => {
                    mismatched.push(digest);
                    reads[index] = Err(mismatched_store_data(format!(
                        "events of transaction {digest} do not match the digest in its effects"
                    )));
                }
                // The store holds the events of a transaction with the transaction
                // itself, so it lacks those of a pruned one only when it fails.
                None if pending.pruned.is_some() => {
                    reads[index] = Err(store_unavailable(format!(
                        "events of transaction {digest} missing after its checkpoint lookup"
                    )))
                }
                None => reads[index] = Err(events_unavailable(&digest, true)),
            }
        }
        if !mismatched.is_empty() {
            store.evict_events_by_tx_digests(&mismatched).await;
        }
    }

    /// Fails a read with `FAILED_PRECONDITION` if an object is unavailable,
    /// `UNAVAILABLE` if the store read fails, and `INTERNAL` if the store
    /// returns another object.
    async fn read_objects(
        &self,
        reads: &mut [Result<PendingRead, RpcError>],
        needs: &ReadNeeds,
        store_budget: &StoreReadBudget,
    ) {
        let mut objects: Vec<Option<ReadObjects>> = Vec::with_capacity(reads.len());
        let mut missing = Vec::new();
        for (index, read) in reads.iter_mut().enumerate() {
            let found = match read {
                Ok(pending) => self.local_objects(pending, needs),
                Err(_) => Ok(None),
            };
            match found {
                Ok(Some((keys, found))) => {
                    if found.iter().any(Option::is_none) {
                        missing.push(index);
                    }
                    objects.push(Some((keys, found)));
                }
                Ok(None) => objects.push(None),
                Err(error) => {
                    *read = Err(error);
                    objects.push(None);
                }
            }
        }

        if let Some(store) = self
            .transaction_fallback
            .as_ref()
            .filter(|_| !missing.is_empty())
        {
            let mut missing_keys = missing
                .iter()
                .filter_map(|&index| objects[index].as_ref())
                .flat_map(|(keys, found)| {
                    keys.iter()
                        .zip(found)
                        .filter(|(_, object)| object.is_none())
                        .map(|(key, _)| ObjectKey::from(key))
                })
                .collect::<Vec<_>>();
            missing_keys.sort_unstable();
            missing_keys.dedup();
            let fetched =
                match store_read(store.multi_get_objects(&missing_keys), store_budget).await {
                    Ok(fetched) => missing_keys
                        .into_iter()
                        .zip(fetched)
                        .filter_map(|(key, object)| Some((key, object?)))
                        .collect::<std::collections::HashMap<_, _>>(),
                    Err(error) => {
                        fail_reads(reads, &missing, &error);
                        std::collections::HashMap::new()
                    }
                };

            let mut mismatched = std::collections::BTreeSet::new();
            for &index in &missing {
                let Some((keys, found)) = &mut objects[index] else {
                    continue;
                };
                for (key, object) in keys.iter().zip(found.iter_mut()) {
                    if object.is_some() {
                        continue;
                    }
                    let Some(fetched) = fetched.get(&ObjectKey::from(key)) else {
                        continue;
                    };
                    if let Err(error) = verify_object(key, fetched) {
                        mismatched.insert(ObjectKey::from(key));
                        if reads[index].is_ok() {
                            reads[index] = Err(error);
                        }
                    }
                    *object = Some(fetched.clone());
                }
            }
            if !mismatched.is_empty() {
                // Every rejected object is read again, so one request clears
                // them all from the store's cache.
                store
                    .evict_objects(&mismatched.into_iter().collect::<Vec<_>>())
                    .await;
            }
        }

        for (read, objects) in reads.iter_mut().zip(objects) {
            let (Ok(pending), Some((keys, found))) = (&mut *read, objects) else {
                continue;
            };
            let input_count = pending.input_count;
            match keys
                .iter()
                .zip(found)
                .map(|(key, object)| {
                    object
                        .ok_or_else(|| object_unavailable(key, self.transaction_fallback.is_some()))
                })
                .collect::<Result<Vec<_>, _>>()
            {
                Ok(mut found) => {
                    let output_objects = found.split_off(input_count);
                    pending.data.input_objects = needs.input_objects.then_some(found);
                    pending.data.output_objects = needs.output_objects.then_some(output_objects);
                }
                Err(error) => *read = Err(error),
            }
        }
    }

    /// The object keys a read needs and the objects the node holds of them; `None` without
    /// effects. Without a store, a missing object fails the read.
    fn local_objects(
        &self,
        pending: &mut PendingRead,
        needs: &ReadNeeds,
    ) -> Result<Option<ReadObjects>, RpcError> {
        let Some(effects) = &pending.data.effects else {
            return Ok(None);
        };
        let mut keys = Vec::new();
        if needs.input_objects {
            keys.extend(
                effects
                    .old_object_metadata()
                    .into_iter()
                    .map(|modified| *modified.reference()),
            );
        }
        pending.input_count = keys.len();
        if needs.output_objects {
            keys.extend(
                effects
                    .created()
                    .into_iter()
                    .chain(effects.mutated())
                    .chain(effects.unwrapped())
                    .map(|written| *written.reference()),
            );
        }
        let mut found = Vec::with_capacity(keys.len());
        for key in &keys {
            let object = self
                .state_reader
                .try_get_object_by_key(&key.object_id, key.version)?;
            if object.is_none() && self.transaction_fallback.is_none() {
                return Err(object_unavailable(key, false));
            }
            found.push(object);
        }
        Ok(Some((keys, found)))
    }
}

/// Reads the transactions and effects of the pruned reads from `store`.
async fn read_pruned_transactions(
    store: &Arc<dyn TransactionKeyValueStoreTrait + Send + Sync>,
    reads: &mut [Result<PendingRead, RpcError>],
    needs: &ReadNeeds,
    store_budget: &StoreReadBudget,
) {
    // A pruned transaction reads both its transaction and its effects from the store.
    let transaction_indexes = pending_indexes(reads, |pending| {
        pending.pruned.is_some() && needs.transaction
    });
    let effects_indexes =
        pending_indexes(reads, |pending| pending.pruned.is_some() && needs.effects);
    if transaction_indexes.is_empty() && effects_indexes.is_empty() {
        return;
    }
    let (transactions, effects) = match store_read(
        store.multi_get(
            &digests_at(reads, &transaction_indexes),
            &digests_at(reads, &effects_indexes),
        ),
        store_budget,
    )
    .await
    {
        Ok(read) => read,
        Err(error) => {
            fail_reads(reads, &transaction_indexes, &error);
            return fail_reads(reads, &effects_indexes, &error);
        }
    };
    let mut transactions = transactions.into_iter();
    for &index in &transaction_indexes {
        let transaction = transactions.next().flatten();
        if let Ok(pending) = &mut reads[index] {
            match transaction {
                Some(transaction) => pending.set_transaction(&transaction, &needs.fields),
                None => pending.has_transaction = false,
            }
        }
    }
    for (&index, effects) in effects_indexes.iter().zip(effects) {
        if let Ok(pending) = &mut reads[index] {
            pending.data.effects = effects;
        }
    }
}

/// The object keys of a read, inputs first, and the objects found of them so far.
type ReadObjects = (Vec<ObjectReference>, Vec<Option<Object>>);

/// Which data the read fields need.
struct ReadNeeds {
    fields: TransactionReadFields,
    transaction: bool,
    effects: bool,
    checkpoint: bool,
    input_objects: bool,
    output_objects: bool,
}

impl ReadNeeds {
    fn new(fields: &TransactionReadFields) -> Self {
        // Derived change fields need effects plus the input/output objects
        let derived_changes = fields.include_balance_changes || fields.include_object_changes;
        let input_objects = fields.include_input_objects || derived_changes;
        let output_objects = fields.include_output_objects || derived_changes;
        Self {
            fields: *fields,
            transaction: fields.include_transaction
                || fields.include_signatures
                || fields.include_object_changes,
            effects: fields.include_effects
                || fields.include_events
                || input_objects
                || output_objects,
            checkpoint: fields.include_checkpoint || fields.include_timestamp,
            input_objects,
            output_objects,
        }
    }
}

/// A transaction read in progress.
struct PendingRead {
    data: TransactionReadData,
    has_transaction: bool,
    /// The checkpoint of a transaction the node has pruned.
    pruned: Option<u64>,
    input_count: usize,
}

impl PendingRead {
    fn new(digest: TransactionDigest) -> Self {
        Self {
            data: TransactionReadData {
                digest,
                transaction: None,
                signatures: None,
                effects: None,
                events: None,
                checkpoint: None,
                timestamp_ms: None,
                input_objects: None,
                output_objects: None,
            },
            has_transaction: false,
            pruned: None,
            input_count: 0,
        }
    }

    fn set_transaction(
        &mut self,
        transaction: &iota_sdk_types::SenderSignedTransaction,
        fields: &TransactionReadFields,
    ) {
        self.has_transaction = true;
        self.data.transaction = (fields.include_transaction || fields.include_object_changes)
            .then(|| transaction.transaction().clone());
        self.data.signatures = fields
            .include_signatures
            .then(|| transaction.signatures().to_owned());
    }

    fn check_found(&self, needs: &ReadNeeds) -> Result<(), RpcError> {
        if (!needs.transaction || self.has_transaction)
            && (!needs.effects || self.data.effects.is_some())
        {
            return Ok(());
        }
        // The store has placed a pruned transaction in a checkpoint, so its
        // transaction or effects missing from the store is a store failure.
        Err(match self.pruned {
            Some(_) => store_unavailable(format!(
                "transaction {} or its effects missing after its checkpoint lookup",
                self.data.digest
            )),
            None => crate::error::TransactionNotFoundError(self.data.digest).into(),
        })
    }

    fn finish(mut self, needs: &ReadNeeds) -> TransactionReadData {
        if !needs.effects {
            self.data.effects = None;
        }
        self.data
    }
}

/// The indexes of the reads that have not failed and match `filter`.
fn pending_indexes(
    reads: &[Result<PendingRead, RpcError>],
    filter: impl Fn(&PendingRead) -> bool,
) -> Vec<usize> {
    reads
        .iter()
        .enumerate()
        .filter_map(|(index, read)| {
            read.as_ref()
                .ok()
                .filter(|pending| filter(pending))
                .map(|_| index)
        })
        .collect()
}

fn digests_at(
    reads: &[Result<PendingRead, RpcError>],
    indexes: &[usize],
) -> Vec<TransactionDigest> {
    indexes
        .iter()
        .filter_map(|&index| Some(reads[index].as_ref().ok()?.data.digest))
        .collect()
}

/// Fails each read at `indexes` that has not failed yet with `error`.
fn fail_reads(reads: &mut [Result<PendingRead, RpcError>], indexes: &[usize], error: &RpcError) {
    for &index in indexes {
        if reads[index].is_ok() {
            reads[index] = Err(error.clone());
        }
    }
}

/// Why data is unavailable, and what to try first. The key-value store reports a failed read as a
/// missing key, so with a store the data may also be missing because the read failed.
fn unavailable_reason(store_read: bool) -> &'static str {
    if store_read {
        "(possibly pruned, or the key-value store failed to return it); retry, and if it stays \
         unavailable, "
    } else {
        "(possibly pruned); "
    }
}

fn events_unavailable(digest: &TransactionDigest, store_read: bool) -> RpcError {
    RpcError::new(
        tonic::Code::FailedPrecondition,
        format!(
            "events of transaction {digest} required by the requested fields are unavailable \
             {}narrow the read_mask",
            unavailable_reason(store_read)
        ),
    )
}

fn object_unavailable(key: &ObjectReference, store_read: bool) -> RpcError {
    let (object_id, version) = (key.object_id, key.version);
    // An incomplete set would corrupt the derived change fields.
    RpcError::new(
        tonic::Code::FailedPrecondition,
        format!(
            "object {object_id} at version {version} required by the requested fields is \
             unavailable {}narrow the read_mask or fetch objects individually via \
             `get_objects` for best-effort retrieval",
            unavailable_reason(store_read)
        ),
    )
}

fn verify_object(
    reference: &ObjectReference,
    object: &Object,
) -> Result<(), crate::error::RpcError> {
    let object = object.as_inner();
    if object.digest() == reference.digest {
        return Ok(());
    }
    Err(mismatched_store_data(format!(
        "key-value store returned object {} at version {} for object {} at version {}, \
         which does not match the digest in the effects",
        object.id(),
        object.version(),
        reference.object_id,
        reference.version
    )))
}

const STORE_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// The time the key-value store reads of one request may take in total.
const STORE_READ_BUDGET: std::time::Duration = std::time::Duration::from_secs(30);

/// The store read time a request has left; shared by its transactions.
///
/// Only time inside store reads counts. A deadline per request would also count the time the client
/// takes to read the response, so a slow client would get `UNAVAILABLE` for its later transactions
/// while the store is healthy.
#[derive(Debug)]
pub struct StoreReadBudget(std::sync::Mutex<std::time::Duration>);

impl Default for StoreReadBudget {
    fn default() -> Self {
        Self(std::sync::Mutex::new(STORE_READ_BUDGET))
    }
}

impl StoreReadBudget {
    fn lock(&self) -> std::sync::MutexGuard<'_, std::time::Duration> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn remaining(&self) -> std::time::Duration {
        *self.lock()
    }

    fn is_spent(&self) -> bool {
        self.remaining().is_zero()
    }

    fn spend(&self, spent: std::time::Duration) {
        let mut remaining = self.lock();
        *remaining = remaining.saturating_sub(spent);
    }
}

async fn store_read<T>(
    read: impl std::future::Future<Output = iota_types::error::IotaResult<T>>,
    budget: &StoreReadBudget,
) -> Result<T, crate::error::RpcError> {
    let remaining = budget.remaining();
    if remaining.is_zero() {
        return Err(store_budget_spent());
    }
    let start = tokio::time::Instant::now();
    let result = tokio::time::timeout(remaining.min(STORE_READ_TIMEOUT), read).await;
    budget.spend(start.elapsed());
    match result {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(store_unavailable(error)),
        Err(_) => Err(store_unavailable("read timed out")),
    }
}

fn store_unavailable(detail: impl std::fmt::Display) -> crate::error::RpcError {
    // The store's error text can name its URL, so it stays in the log.
    tracing::warn!("key-value store read failed: {detail}");
    crate::error::RpcError::new(tonic::Code::Unavailable, "key-value store unavailable")
}

/// Not logged: every further item of the request gets it, and no read failed.
fn store_budget_spent() -> crate::error::RpcError {
    crate::error::RpcError::new(
        tonic::Code::Unavailable,
        "key-value store read time of the request is spent; request fewer transactions",
    )
}

fn mismatched_store_data(detail: String) -> crate::error::RpcError {
    tracing::warn!("{detail}");
    crate::error::RpcError::new(
        tonic::Code::Internal,
        "key-value store returned data that does not match the effects",
    )
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
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn store_read_that_never_completes_is_unavailable() {
        let budget = StoreReadBudget::default();
        let never = || std::future::pending::<iota_types::error::IotaResult<()>>();

        for _ in 0..3 {
            let start = tokio::time::Instant::now();
            let error = store_read(never(), &budget).await.unwrap_err();
            assert_eq!(Status::from(error).code(), tonic::Code::Unavailable);
            assert_eq!(start.elapsed(), std::time::Duration::from_secs(10));
        }
        // Three reads of 10 s each spend the 30 s budget.
        assert!(budget.is_spent());
    }

    #[tokio::test(start_paused = true)]
    async fn store_read_waits_no_longer_than_the_budget_left() {
        let budget = StoreReadBudget::default();
        budget.spend(std::time::Duration::from_secs(25));
        let start = tokio::time::Instant::now();

        let never = std::future::pending::<iota_types::error::IotaResult<()>>();
        store_read(never, &budget).await.unwrap_err();

        assert_eq!(start.elapsed(), std::time::Duration::from_secs(5));
        assert!(budget.is_spent());
    }

    #[tokio::test(start_paused = true)]
    async fn successful_store_reads_spend_the_budget() {
        let budget = StoreReadBudget::default();
        for _ in 0..3 {
            let slow = async {
                tokio::time::sleep(std::time::Duration::from_secs(9)).await;
                Ok(())
            };
            store_read(slow, &budget).await.unwrap();
        }
        assert_eq!(budget.remaining(), std::time::Duration::from_secs(3));
    }

    #[tokio::test(start_paused = true)]
    async fn time_outside_store_reads_does_not_spend_the_budget() {
        let budget = StoreReadBudget::default();
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        store_read(async { Ok(()) }, &budget).await.unwrap();
        assert!(!budget.is_spent());
    }

    #[tokio::test(start_paused = true)]
    async fn store_read_past_the_budget_is_unavailable_without_reading() {
        let budget = StoreReadBudget::default();
        budget.spend(std::time::Duration::from_secs(30));
        let read = async { panic!("the store must not be read past the budget") };
        let error = store_read::<()>(read, &budget).await.unwrap_err();
        assert_eq!(Status::from(error).code(), tonic::Code::Unavailable);
    }
}
