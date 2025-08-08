// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use anyhow::Result;
use iota_grpc_types::{
    CertifiedCheckpointSummary as GrpcCertifiedCheckpointSummary,
    CheckpointData as GrpcCheckpointData,
};
use iota_types::{
    full_checkpoint_content::CheckpointData, messages_checkpoint::CertifiedCheckpointSummary,
    storage::RestStateReader,
};
use serde::{Deserialize, Serialize};
use tonic::Status;

use crate::checkpoint::{BcsData, Checkpoint};

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
    sender: tokio::sync::broadcast::Sender<Arc<GrpcCertifiedCheckpointSummary>>,
}

impl GrpcCheckpointSummaryBroadcaster {
    pub fn new(
        sender: tokio::sync::broadcast::Sender<Arc<GrpcCertifiedCheckpointSummary>>,
    ) -> Self {
        Self { sender }
    }

    /// Subscribe to checkpoint summary broadcasts
    pub fn subscribe(
        &self,
    ) -> tokio::sync::broadcast::Receiver<Arc<GrpcCertifiedCheckpointSummary>> {
        self.sender.subscribe()
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
    sender: tokio::sync::broadcast::Sender<Arc<GrpcCheckpointData>>,
}

impl GrpcCheckpointDataBroadcaster {
    pub fn new(sender: tokio::sync::broadcast::Sender<Arc<GrpcCheckpointData>>) -> Self {
        Self { sender }
    }

    /// Subscribe to checkpoint data broadcasts
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<Arc<GrpcCheckpointData>> {
        self.sender.subscribe()
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
impl CheckpointSummaryBroadcaster
    for tokio::sync::broadcast::Sender<Arc<CertifiedCheckpointSummary>>
{
    fn send(&self, summary: &CertifiedCheckpointSummary) -> anyhow::Result<()> {
        self.send(Arc::new(summary.clone()))?;
        Ok(())
    }
}

/// Implementation for tokio broadcast sender
impl CheckpointDataBroadcaster for tokio::sync::broadcast::Sender<Arc<CheckpointData>> {
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

type Receiver<T> = tokio::sync::broadcast::Receiver<T>;

impl BcsData {
    pub fn serialize_from<T>(data: &T) -> Result<Self, bcs::Error>
    where
        T: Serialize,
    {
        let serialized = bcs::to_bytes(data)?;
        Ok(BcsData { data: serialized })
    }

    pub fn deserialize_into<T>(&self) -> Result<T, bcs::Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        bcs::from_bytes(&self.data)
    }
}

// Type aliases and utility types
pub type CheckpointStreamResult = Result<Checkpoint, Status>;

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

// Helper trait for getting checkpoint data and summaries,
// intended as an abstraction for checkpoint streaming operations.
pub trait CheckpointReader<T>
where
    T: Send + Sync + 'static + Serialize,
    Self: Send + Sync + 'static,
{
    fn get_sequence_number(&self, item: &Arc<T>) -> u64;
    fn get_item(&self, seq: u64) -> Option<Arc<T>>;
    fn get_latest(&self) -> Option<u64>;

    fn create_checkpoint_response(&self, item: &Arc<T>, is_full: bool) -> CheckpointStreamResult {
        BcsData::serialize_from(item)
            .map(|data| Checkpoint {
                sequence_number: self.get_sequence_number(item),
                bcs_data: Some(data),
                is_full,
            })
            .map_err(|e| Status::internal(format!("BCS serialization error: {e}")))
    }

    /// Create a checkpoint stream combining historical data from storage and
    /// live data from broadcasts
    fn create_checkpoint_stream(
        self,
        mut rx: Receiver<Arc<T>>,
        start_sequence_number: Option<u64>,
        end_sequence_number: Option<u64>,
        is_full: bool,
    ) -> impl futures::Stream<Item = CheckpointStreamResult> + Send + 'static
    where
        Self: Send + Sync + 'static + Sized,
    {
        let reader = self;
        async_stream::try_stream! {
            let mut latest = reader.get_latest().unwrap_or(0);
            tracing::debug!("[profile][grpc] Latest checkpoint index: {latest}.");
            let (mut start, end) = match (start_sequence_number, end_sequence_number) {
                (None, None) => (latest, u64::MAX),
                (None, Some(end)) => (end, end),
                (Some(start), None) => (start, u64::MAX),
                (Some(start), Some(end)) => (start, end),
            };

            while start <= end {
                // try fetching historical data from the DB first
                if start <= latest {
                    if let Some(item) = reader.get_item(start) {
                        tracing::debug!("[profile][grpc] Fetched checkpoint data for index {start} from storage.");
                        yield reader.create_checkpoint_response(&item, is_full)?;
                        if start == end {
                            break;
                        }
                        start += 1;
                        continue;
                    } else {
                        Err(Status::internal(format!("Historical checkpoint data missing/pruned: index={start} latest={latest}.")))?;
                    }
                }

                // latest < start, live phase
                // wait for broadcast
                match rx.recv().await {
                    Ok(item) => {
                        tracing::debug!("[profile][grpc] Get checkpoint data for index {} from broadcast channel", reader.get_sequence_number(&item));
                        let seq_number = reader.get_sequence_number(&item);
                        if start == seq_number {
                            yield reader.create_checkpoint_response(&item, is_full)?;
                            if start == end {
                                break;
                            }
                            start += 1;
                            continue;
                        }
                        // else item sequence doesn't match, drop it and continue
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        // continue, lagged item should be picked up from history DB
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        // report internal error to the stream and break
                        Err(Status::internal("Checkpoint data channel closed."))?;
                        break;
                    },
                }

                // Update latest checkpoint info
                latest = reader.get_latest().unwrap_or(start);
                tracing::debug!("[profile][grpc] Updating latest checkpoint index to {latest}.");
            }
        }
    }
}

/// Adapter that implements GrpcStateReader for RestStateReader
pub struct RestStateReaderAdapter {
    inner: Arc<dyn RestStateReader>,
}

impl GrpcStateReader for RestStateReaderAdapter {
    fn get_latest_checkpoint_sequence_number(&self) -> Option<u64> {
        Some(*self.inner.get_latest_checkpoint().sequence_number())
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

/// Central gRPC data reader that provides unified access to checkpoint data. It
/// implements `CheckpointReader` traits to support different types of
/// checkpoint streaming.
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
}

/// CheckpointReader implementation for streaming full checkpoint data
impl CheckpointReader<GrpcCheckpointData> for GrpcReader {
    fn get_sequence_number(&self, item: &Arc<GrpcCheckpointData>) -> u64 {
        item.sequence_number()
    }

    fn get_item(&self, seq: u64) -> Option<Arc<GrpcCheckpointData>> {
        self.get_full_checkpoint_data(seq)
            .map(GrpcCheckpointData::from)
            .map(Arc::new)
    }

    fn get_latest(&self) -> Option<u64> {
        self.state_reader.get_latest_checkpoint_sequence_number()
    }
}

/// CheckpointReader implementation for streaming checkpoint summaries
impl CheckpointReader<GrpcCertifiedCheckpointSummary> for GrpcReader {
    fn get_sequence_number(&self, item: &Arc<GrpcCertifiedCheckpointSummary>) -> u64 {
        item.sequence_number()
    }

    fn get_item(&self, ix: u64) -> Option<Arc<GrpcCertifiedCheckpointSummary>> {
        self.state_reader
            .get_checkpoint_summary(ix)
            .map(GrpcCertifiedCheckpointSummary::from)
            .map(Arc::new)
    }

    fn get_latest(&self) -> Option<u64> {
        self.state_reader.get_latest_checkpoint_sequence_number()
    }
}
