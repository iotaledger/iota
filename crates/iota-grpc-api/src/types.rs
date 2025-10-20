// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use anyhow::Result;
use iota_grpc_types::{
    checkpoints::{
        CertifiedCheckpointSummary as GrpcCertifiedCheckpointSummary,
        CheckpointData as GrpcCheckpointData,
    },
    v0::{checkpoints as grpc_checkpoints, common as grpc_common},
};
use iota_json_rpc_types::{EventFilter, IotaEvent};
use iota_types::{
    full_checkpoint_content::CheckpointData,
    messages_checkpoint::CertifiedCheckpointSummary,
    storage::{RestStateReader, error::Kind},
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

/// Trait for subscribing to event streams (used by gRPC service)
pub trait EventSubscriber: Send + Sync {
    /// Subscribe to events with the given filter
    fn subscribe_events(
        &self,
        filter: EventFilter,
    ) -> Box<dyn futures::Stream<Item = IotaEvent> + Send + Unpin>;
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

/// No-op implementation for unit type (used in tests and when event
/// subscription is not needed)
impl EventSubscriber for () {
    fn subscribe_events(
        &self,
        _filter: EventFilter,
    ) -> Box<dyn futures::Stream<Item = IotaEvent> + Send + Unpin> {
        Box::new(Box::pin(futures::stream::empty()))
    }
}

// Type aliases and utility types
pub type CheckpointStreamResult = Result<grpc_checkpoints::Checkpoint, Status>;

// Storage abstraction traits for gRPC access
// These traits provide an abstraction layer over the storage backend,
// making it easier to implement gRPC services with different storage types
// (e.g., production database vs simulacrum for testing).

/// Trait for reading checkpoint data from storage
pub trait GrpcStateReader: Send + Sync + 'static {
    /// Get the latest checkpoint sequence number
    fn get_latest_checkpoint_sequence_number(&self) -> Option<u64>;

    /// Get checkpoint summary by sequence number
    fn get_checkpoint_summary(&self, seq: u64) -> Option<CertifiedCheckpointSummary>;

    /// Get full checkpoint data by sequence number
    fn get_checkpoint_data(&self, seq: u64) -> Option<CheckpointData>;

    /// Get epoch's last checkpoint for epoch boundary calculations
    fn get_epoch_last_checkpoint(
        &self,
        epoch: u64,
    ) -> anyhow::Result<Option<CertifiedCheckpointSummary>>;
}

/// Adapter that implements GrpcStateReader for RestStateReader
pub struct RestStateReaderAdapter {
    inner: Arc<dyn RestStateReader>,
}

impl GrpcStateReader for RestStateReaderAdapter {
    fn get_latest_checkpoint_sequence_number(&self) -> Option<u64> {
        match self.inner.try_get_latest_checkpoint() {
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

    fn get_checkpoint_summary(&self, seq: u64) -> Option<CertifiedCheckpointSummary> {
        self.inner
            .get_checkpoint_by_sequence_number(seq)
            .map(CertifiedCheckpointSummary::from)
    }

    fn get_checkpoint_data(&self, seq: u64) -> Option<CheckpointData> {
        let summary = self.inner.get_checkpoint_by_sequence_number(seq)?;
        let contents = self.inner.get_checkpoint_contents_by_sequence_number(seq)?;
        Some(self.inner.get_checkpoint_data(summary, contents))
    }

    fn get_epoch_last_checkpoint(
        &self,
        epoch: u64,
    ) -> anyhow::Result<Option<CertifiedCheckpointSummary>> {
        match self.inner.get_epoch_last_checkpoint(epoch) {
            Ok(Some(checkpoint)) => Ok(Some(CertifiedCheckpointSummary::from(checkpoint))),
            Ok(None) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

/// Central gRPC data reader that provides unified access to checkpoint data.
/// It provides methods for streaming both full checkpoint data and checkpoint
/// summaries.
#[derive(Clone)]
pub struct GrpcReader {
    state_reader: Arc<dyn GrpcStateReader>,
}

impl GrpcReader {
    pub fn new(state_reader: Arc<dyn GrpcStateReader>) -> Self {
        Self { state_reader }
    }

    pub fn from_rest_state_reader(state_reader: Arc<dyn RestStateReader>) -> Self {
        Self {
            state_reader: Arc::new(RestStateReaderAdapter {
                inner: state_reader,
            }),
        }
    }

    pub fn get_epoch_last_checkpoint(
        &self,
        epoch: u64,
    ) -> anyhow::Result<Option<CertifiedCheckpointSummary>> {
        self.state_reader.get_epoch_last_checkpoint(epoch)
    }

    fn get_full_checkpoint_data(&self, seq: u64) -> Option<CheckpointData> {
        self.state_reader.get_checkpoint_data(seq)
    }

    pub fn get_latest_checkpoint_sequence_number(&self) -> Option<u64> {
        self.state_reader.get_latest_checkpoint_sequence_number()
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
                        debug!("[profile][grpc] Checkpoint {data_type_name} stream cancelled");
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
                        Err(Status::internal(format!("Checkpoint {data_type_name} channel closed.")))?;
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
                    .state_reader
                    .get_checkpoint_summary(seq)
                    .map(GrpcCertifiedCheckpointSummary::from)
                    .map(Arc::new)
            },
            |item| item.sequence_number(),
        )
    }
}
