// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

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
    fn send(
        &self,
        summary: &CertifiedCheckpointSummary,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Trait for broadcasting checkpoint data
pub trait CheckpointDataBroadcaster {
    fn send(&self, data: &CheckpointData) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
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
}

impl CheckpointSummaryBroadcaster for GrpcCheckpointSummaryBroadcaster {
    fn send(
        &self,
        summary: &CertifiedCheckpointSummary,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let grpc_summary = Arc::new(GrpcCertifiedCheckpointSummary::from(summary.clone()));
        self.sender
            .send(grpc_summary)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
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
}

impl CheckpointDataBroadcaster for GrpcCheckpointDataBroadcaster {
    fn send(&self, data: &CheckpointData) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let grpc_data = Arc::new(GrpcCheckpointData::from(data.clone()));
        self.sender
            .send(grpc_data)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        Ok(())
    }
}

// Standard implementations for common types

/// Implementation for tokio broadcast sender
impl CheckpointSummaryBroadcaster
    for tokio::sync::broadcast::Sender<Arc<CertifiedCheckpointSummary>>
{
    fn send(
        &self,
        summary: &CertifiedCheckpointSummary,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.send(Arc::new(summary.clone()))
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        Ok(())
    }
}

/// Implementation for tokio broadcast sender
impl CheckpointDataBroadcaster for tokio::sync::broadcast::Sender<Arc<CheckpointData>> {
    fn send(&self, data: &CheckpointData) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.send(Arc::new(data.clone()))
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        Ok(())
    }
}

/// No-op implementation for unit type (used in tests and when broadcasting is
/// disabled)
impl CheckpointSummaryBroadcaster for () {
    fn send(
        &self,
        _summary: &CertifiedCheckpointSummary,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

/// No-op implementation for unit type (used in tests and when broadcasting is
/// disabled)
impl CheckpointDataBroadcaster for () {
    fn send(&self, _data: &CheckpointData) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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

// Helper trait for getting checkpoint data and summaries,
// intended as an abstraction for Arc<dyn RestStateReader>.
pub trait CheckpointReader<T>
where
    T: Send + Sync + 'static + Serialize,
    Self: Send + Sync + 'static,
{
    fn get_sequence_number(&self, item: &Arc<T>) -> u64;
    fn get_item(&self, ix: u64) -> Option<Arc<T>>;
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
}

#[derive(Clone)]
pub struct Reader {
    pub state_reader: Arc<dyn RestStateReader>,
}

impl Reader {
    fn get_full_checkpoint_data(&self, seq: u64) -> Option<CheckpointData> {
        let summary = self.state_reader.get_checkpoint_by_sequence_number(seq)?;
        let contents = self
            .state_reader
            .get_checkpoint_contents_by_sequence_number(seq)?;
        Some(self.state_reader.get_checkpoint_data(summary, contents))
    }

    pub fn create_checkpoint_stream<T>(
        &self,
        mut rx: Receiver<Arc<T>>,
        start_sequence_number: Option<u64>,
        end_sequence_number: Option<u64>,
        is_full: bool,
    ) -> impl futures::Stream<Item = CheckpointStreamResult> + Send
    where
        T: Send + Sync + 'static + Serialize,
        Self: CheckpointReader<T> + Clone + Send + Sync + 'static,
    {
        let reader = self.clone();
        async_stream::try_stream! {
            // Link to issue (https://github.com/iotaledger/iota/issues/7943)
            // TODO: Modify the latest checkpoint to start from 1.
            // Note that we do not stream the Genesis checkpoint because its size
            // can be very big. The genesis checkpoint should be imported directly.
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
                        tracing::debug!("[profile][grpc] Fetched checkpoint data for index {start} from DB.");
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
                latest = reader.get_latest().unwrap_or(start);
                tracing::debug!("[profile][grpc] Updating latest checkpoint index to {latest}.");
            }
        }
    }
}

impl CheckpointReader<GrpcCheckpointData> for Reader {
    fn get_sequence_number(&self, item: &Arc<GrpcCheckpointData>) -> u64 {
        item.sequence_number()
    }
    fn get_item(&self, ix: u64) -> Option<Arc<GrpcCheckpointData>> {
        self.get_full_checkpoint_data(ix)
            .map(GrpcCheckpointData::from)
            .map(Arc::new)
    }
    fn get_latest(&self) -> Option<u64> {
        Some(*self.state_reader.get_latest_checkpoint().sequence_number())
    }
}

impl CheckpointReader<GrpcCertifiedCheckpointSummary> for Reader {
    fn get_sequence_number(&self, item: &Arc<GrpcCertifiedCheckpointSummary>) -> u64 {
        item.sequence_number()
    }
    fn get_item(&self, ix: u64) -> Option<Arc<GrpcCertifiedCheckpointSummary>> {
        self.state_reader
            .get_checkpoint_by_sequence_number(ix)
            .map(|v| GrpcCertifiedCheckpointSummary::from(CertifiedCheckpointSummary::from(v)))
            .map(Arc::new)
    }
    fn get_latest(&self) -> Option<u64> {
        Some(*self.state_reader.get_latest_checkpoint().sequence_number())
    }
}
