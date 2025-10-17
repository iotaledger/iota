// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use anyhow::Result;
use iota_core::{
    authority::{AuthorityState, authority_per_epoch_store::AuthorityPerEpochStore},
    storage::RestReadStore,
};
use iota_grpc_types::{
    checkpoints::{
        CertifiedCheckpointSummary as GrpcCertifiedCheckpointSummary,
        CheckpointData as GrpcCheckpointData,
    },
    v0::{checkpoints as grpc_checkpoints, common as grpc_common},
};
use iota_storage::key_value_store::TransactionKeyValueStore;
use iota_types::{
    base_types::ObjectID,
    full_checkpoint_content::CheckpointData,
    messages_checkpoint::CertifiedCheckpointSummary,
    object::{Object, ObjectRead},
    storage::{ReadStore, RestStateReader, error::Kind},
};
use serde::Serialize;
use tokio::sync::broadcast::{Receiver, Sender, error::RecvError};
use tokio_util::sync::CancellationToken;
use tonic::Status;
use tracing::debug;

/// Trait for broadcasting checkpoint summaries
pub trait CheckpointSummaryBroadcaster {
    fn send(&self, summary: &CertifiedCheckpointSummary) -> anyhow::Result<()>;
}

/// Trait for broadcasting checkpoint data
pub trait CheckpointDataBroadcaster {
    fn send(&self, data: &CheckpointData) -> anyhow::Result<()>;
}

/// Wrapper that converts native CertifiedCheckpointSummary to gRPC type before
/// broadcasting
#[derive(Clone)]
pub struct GrpcCheckpointSummaryBroadcaster {
    sender: Sender<Arc<GrpcCertifiedCheckpointSummary>>,
}

impl GrpcCheckpointSummaryBroadcaster {
    pub fn new(sender: Sender<Arc<GrpcCertifiedCheckpointSummary>>) -> Self {
        Self { sender }
    }

    /// Subscribe to checkpoint summary broadcasts
    pub fn subscribe(&self) -> Receiver<Arc<GrpcCertifiedCheckpointSummary>> {
        self.sender.subscribe()
    }

    /// Get the number of active receivers
    pub fn receiver_count(&self) -> usize {
        self.sender.receiver_count()
    }

    /// Send with integrated tracing and error handling
    pub fn send_traced(&self, summary: &CertifiedCheckpointSummary) {
        match self.send(summary) {
            Ok(()) => {
                debug!(
                    "Sent checkpoint summary #{} to {} gRPC subscriber(s)",
                    *summary.data().sequence_number(),
                    self.receiver_count()
                );
            }
            Err(_) => {
                debug!(
                    "No gRPC clients subscribed for checkpoint summary #{}",
                    *summary.data().sequence_number()
                );
            }
        }
    }
}

impl CheckpointSummaryBroadcaster for GrpcCheckpointSummaryBroadcaster {
    fn send(&self, summary: &CertifiedCheckpointSummary) -> anyhow::Result<()> {
        let grpc_summary = Arc::new(GrpcCertifiedCheckpointSummary::from(summary.clone()));
        self.sender.send(grpc_summary)?;
        Ok(())
    }
}

/// Wrapper that converts native CheckpointData to gRPC type before broadcasting
#[derive(Clone)]
pub struct GrpcCheckpointDataBroadcaster {
    sender: Sender<Arc<GrpcCheckpointData>>,
}

impl GrpcCheckpointDataBroadcaster {
    pub fn new(sender: Sender<Arc<GrpcCheckpointData>>) -> Self {
        Self { sender }
    }

    /// Subscribe to checkpoint data broadcasts
    pub fn subscribe(&self) -> Receiver<Arc<GrpcCheckpointData>> {
        self.sender.subscribe()
    }

    /// Get the number of active receivers
    pub fn receiver_count(&self) -> usize {
        self.sender.receiver_count()
    }

    /// Send with integrated tracing and error handling
    pub fn send_traced(&self, data: &CheckpointData) {
        match self.send(data) {
            Ok(()) => {
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

impl CheckpointDataBroadcaster for GrpcCheckpointDataBroadcaster {
    fn send(&self, data: &CheckpointData) -> anyhow::Result<()> {
        let grpc_data = Arc::new(GrpcCheckpointData::from(data.clone()));
        self.sender.send(grpc_data)?;
        Ok(())
    }
}

// Standard implementations for common types

/// Implementation for tokio broadcast sender
impl CheckpointSummaryBroadcaster for Sender<Arc<CertifiedCheckpointSummary>> {
    fn send(&self, summary: &CertifiedCheckpointSummary) -> anyhow::Result<()> {
        self.send(Arc::new(summary.clone()))?;
        Ok(())
    }
}

/// Implementation for tokio broadcast sender
impl CheckpointDataBroadcaster for Sender<Arc<CheckpointData>> {
    fn send(&self, data: &CheckpointData) -> anyhow::Result<()> {
        self.send(Arc::new(data.clone()))?;
        Ok(())
    }
}

/// No-op implementation for unit type (used in tests and when broadcasting is
/// disabled)
impl CheckpointSummaryBroadcaster for () {
    fn send(&self, _summary: &CertifiedCheckpointSummary) -> anyhow::Result<()> {
        Ok(())
    }
}

/// No-op implementation for unit type (used in tests and when broadcasting is
/// disabled)
impl CheckpointDataBroadcaster for () {
    fn send(&self, _data: &CheckpointData) -> anyhow::Result<()> {
        Ok(())
    }
}

// Type aliases and utility types
pub type CheckpointStreamResult = Result<grpc_checkpoints::Checkpoint, Status>;
/// Central gRPC data reader that provides unified access to checkpoint data.
/// It provides methods for streaming both full checkpoint data and checkpoint
/// summaries.
#[derive(Clone)]
pub struct GrpcReader {
    state_reader: Arc<dyn RestStateReader>,
    transaction_kv_store: Option<Arc<TransactionKeyValueStore>>,
}

impl GrpcReader {
    /// Primary constructor for production use with RestReadStore
    pub fn new(
        rest_read_store: Arc<RestReadStore>,
        transaction_kv_store: Option<Arc<TransactionKeyValueStore>>,
    ) -> Self {
        Self {
            state_reader: rest_read_store,
            transaction_kv_store,
        }
    }

    /// Constructor for tests/mocks with generic RestStateReader
    pub fn from_rest_state_reader(
        state_reader: Arc<dyn RestStateReader>,
        transaction_kv_store: Option<Arc<TransactionKeyValueStore>>,
    ) -> Self {
        Self {
            state_reader,
            transaction_kv_store,
        }
    }

    /// Load epoch store for transaction processing with graceful fallback
    pub fn load_epoch_store_one_call_per_task(&self) -> Option<Arc<AuthorityPerEpochStore>> {
        // Use authority_state_any() to access AuthorityState
        self.state_reader
            .authority_state_any()?
            .downcast_ref::<Arc<AuthorityState>>()
            .map(|state| state.load_epoch_store_one_call_per_task().clone())
    }

    /// Get epoch's last checkpoint for epoch boundary calculations with
    /// gRPC-friendly error handling
    pub fn get_epoch_last_checkpoint(
        &self,
        epoch: u64,
    ) -> anyhow::Result<Option<CertifiedCheckpointSummary>> {
        match self.state_reader.get_epoch_last_checkpoint(epoch) {
            Ok(Some(checkpoint)) => Ok(Some(CertifiedCheckpointSummary::from(checkpoint))),
            Ok(None) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get full checkpoint data by sequence number with gRPC-friendly error
    /// handling
    pub fn get_full_checkpoint_data(&self, seq: u64) -> Option<CheckpointData> {
        let summary = self
            .state_reader
            .try_get_checkpoint_by_sequence_number(seq)
            .ok()??;
        let contents = self
            .state_reader
            .try_get_checkpoint_contents_by_sequence_number(seq)
            .ok()??;
        Some(self.state_reader.get_checkpoint_data(summary, contents))
    }

    /// Get checkpoint summary by sequence number with type conversion
    pub fn get_checkpoint_summary(&self, seq: u64) -> Option<CertifiedCheckpointSummary> {
        self.state_reader
            .try_get_checkpoint_by_sequence_number(seq)
            .ok()?
            .map(CertifiedCheckpointSummary::from)
    }

    /// Get the latest checkpoint sequence number with gRPC-friendly error
    /// handling
    pub fn get_latest_checkpoint_sequence_number(&self) -> Option<u64> {
        match self.state_reader.try_get_latest_checkpoint() {
            Ok(checkpoint) => Some(*checkpoint.sequence_number()),
            Err(e) => match e.kind() {
                // Expected during server initialization when no checkpoints have been executed yet
                // Return None to indicate service is not ready rather than panicking
                Kind::Missing => None,
                // Unexpected storage errors
                _ => panic!("Unexpected storage error: {e}"),
            },
        }
    }

    /// Get object data by object ID with anyhow error handling
    pub fn get_object(&self, object_id: &ObjectID) -> anyhow::Result<Option<Object>> {
        match self.state_reader.try_get_object(object_id) {
            Ok(object) => Ok(object),
            Err(e) => Err(e.into()),
        }
    }

    /// Access to authority_state for display fields computation
    pub fn authority_state(&self) -> Option<&Arc<AuthorityState>> {
        // Use authority_state_any() to access AuthorityState
        self.state_reader
            .authority_state_any()?
            .downcast_ref::<Arc<AuthorityState>>()
    }

    /// Access to transaction_kv_store for display fields computation
    pub fn transaction_kv_store(&self) -> &Option<Arc<TransactionKeyValueStore>> {
        &self.transaction_kv_store
    }

    /// Get object with layout information like JSON RPC (when AuthorityState is
    /// available)
    pub fn get_object_read(&self, object_id: &ObjectID) -> anyhow::Result<ObjectRead> {
        match self.authority_state() {
            Some(state) => {
                // Use AuthorityState.get_object_read() for full ObjectRead with layout
                state.get_object_read(object_id).map_err(Into::into)
            }
            None => {
                // Fallback: use basic object access and construct ObjectRead manually
                match self.get_object(object_id)? {
                    Some(object) => {
                        let object_ref = object.compute_object_reference();
                        Ok(ObjectRead::Exists(object_ref, object, None)) // No layout available
                    }
                    None => Ok(ObjectRead::NotExists(*object_id)),
                }
            }
        }
    }

    /// Generic checkpoint streaming implementation that works with checkpoint
    /// data and summaries.
    fn create_checkpoint_stream<T>(
        &self,
        mut rx: Receiver<Arc<T>>,
        start_sequence_number: Option<u64>,
        end_sequence_number: Option<u64>,
        is_full: bool,
        cancellation_token: CancellationToken,
        fetch_historical: impl Fn(&Self, u64) -> Option<Arc<T>> + Send,
        get_sequence_number: impl Fn(&Arc<T>) -> u64 + Send,
    ) -> impl futures::Stream<Item = CheckpointStreamResult> + Send
    where
        T: Serialize + Send + Sync + 'static,
    {
        // Clone self to avoid lifetime issues with the async stream
        let reader = self.clone();
        async_stream::try_stream! {
            let data_type_name = if is_full { "data" } else { "summary" };
            // Link to issue (https://github.com/iotaledger/iota/issues/7943)
            // TODO: Modify the latest checkpoint to start from 1.
            // Note that we do not stream the Genesis checkpoint because its size
            // can be very big. The genesis checkpoint should be imported directly.
            let mut latest = reader.get_latest_checkpoint_sequence_number().unwrap_or(0);
            debug!("[profile][grpc] Latest checkpoint index: {latest}.");
            let (mut start, end) = match (start_sequence_number, end_sequence_number) {
                (None, None) => (latest, u64::MAX),
                (None, Some(end)) => (end, end),
                (Some(start), None) => (start, u64::MAX),
                (Some(start), Some(end)) => (start, end),
            };
            while start <= end {
                // try fetching historical data from the DB first
                if start <= latest {
                    if let Some(item) = fetch_historical(&reader, start) {
                        debug!("[profile][grpc] Fetched checkpoint {data_type_name} for index {start} from DB.");
                        let sequence_number = get_sequence_number(&item);
                        let response = grpc_common::BcsData::serialize_from(&*item)
                            .map(|data| grpc_checkpoints::Checkpoint {
                                sequence_number,
                                bcs_data: Some(data),
                                is_full,
                            })
                            .map_err(|e| Status::internal(format!("BCS serialization error: {e}")))?;
                        yield response;
                        if start == end {
                            break;
                        }
                        start += 1;
                        continue;
                    } else {
                        Err(Status::internal(format!("Historical checkpoint {data_type_name} missing/pruned: index={start} latest={latest}.")))?;
                    }
                }
                // latest < start, live phase
                // wait for broadcast or cancellation
                let item_result = tokio::select! {
                    // note: tokio::select! cannot return results, so we put the match logic after the select
                    recv_result = rx.recv() => Some(recv_result),
                    _ = cancellation_token.cancelled() => {
                        debug!("[profile][grpc] grpc_checkpoints::Checkpoint {data_type_name} stream cancelled");
                        None
                    }
                };

                match item_result {
                    Some(Ok(item)) => {
                        debug!("[profile][grpc] Get checkpoint {data_type_name} for index {} from broadcast channel", get_sequence_number(&item));
                        let sequence_number = get_sequence_number(&item);
                        if start == sequence_number {
                            let response = grpc_common::BcsData::serialize_from(&*item)
                                .map(|data| grpc_checkpoints::Checkpoint {
                                    sequence_number,
                                    bcs_data: Some(data),
                                    is_full,
                                })
                                .map_err(|e| Status::internal(format!("BCS serialization error: {e}")))?;
                            yield response;
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
                        // report internal error to the stream and break
                        Err(Status::internal(format!("grpc_checkpoints::Checkpoint {data_type_name} channel closed.")))?;
                        break;
                    }
                    None => {
                        // Cancellation was triggered
                        break;
                    }
                }
                latest = reader.get_latest_checkpoint_sequence_number().unwrap_or(start);
                debug!("[profile][grpc] Updating latest checkpoint index to {latest}.");
            }
        }
    }

    /// Create a checkpoint stream for full checkpoint data
    pub fn create_checkpoint_data_stream(
        &self,
        rx: Receiver<Arc<GrpcCheckpointData>>,
        start_sequence_number: Option<u64>,
        end_sequence_number: Option<u64>,
        cancellation_token: CancellationToken,
    ) -> impl futures::Stream<Item = CheckpointStreamResult> + Send {
        self.create_checkpoint_stream(
            rx,
            start_sequence_number,
            end_sequence_number,
            true,
            cancellation_token,
            |reader, seq| {
                reader
                    .get_full_checkpoint_data(seq)
                    .map(GrpcCheckpointData::from)
                    .map(Arc::new)
            },
            |item| item.sequence_number(),
        )
    }

    /// Create a checkpoint stream for checkpoint summaries
    pub fn create_checkpoint_summary_stream(
        &self,
        rx: Receiver<Arc<GrpcCertifiedCheckpointSummary>>,
        start_sequence_number: Option<u64>,
        end_sequence_number: Option<u64>,
        cancellation_token: CancellationToken,
    ) -> impl futures::Stream<Item = CheckpointStreamResult> + Send {
        self.create_checkpoint_stream(
            rx,
            start_sequence_number,
            end_sequence_number,
            false,
            cancellation_token,
            |reader, seq| {
                reader
                    .get_checkpoint_summary(seq)
                    .map(GrpcCertifiedCheckpointSummary::from)
                    .map(Arc::new)
            },
            |item| item.sequence_number(),
        )
    }
}
