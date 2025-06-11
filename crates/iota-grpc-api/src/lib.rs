// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{pin::Pin, sync::Arc};

use tonic::{Request, Response, Status};
pub mod checkpoint {
    tonic::include_proto!("iota.grpc");
}

use checkpoint::checkpoint_service_server::CheckpointService;
use iota_types::storage::RestStateReader;
pub mod client;
use async_stream::stream;
use bcs;
use iota_types::{
    full_checkpoint_content::CheckpointData,
    grpc::{
        CertifiedCheckpointSummary as GrpcCertifiedCheckpointSummary,
        CheckpointData as GrpcCheckpointData,
    },
    messages_checkpoint::CertifiedCheckpointSummary,
};

pub struct CheckpointGrpcService {
    pub state_reader: Arc<dyn RestStateReader>,
    pub grpc_checkpoint_summary_tx:
        tokio::sync::broadcast::Sender<Arc<GrpcCertifiedCheckpointSummary>>,
    pub grpc_checkpoint_data_tx: tokio::sync::broadcast::Sender<Arc<GrpcCheckpointData>>,
}

impl CheckpointGrpcService {
    pub fn new(
        state_reader: Arc<dyn RestStateReader>,
        grpc_checkpoint_summary_tx: tokio::sync::broadcast::Sender<
            Arc<GrpcCertifiedCheckpointSummary>,
        >,
        grpc_checkpoint_data_tx: tokio::sync::broadcast::Sender<Arc<GrpcCheckpointData>>,
    ) -> Self {
        Self {
            state_reader,
            grpc_checkpoint_summary_tx,
            grpc_checkpoint_data_tx,
        }
    }
}

// Checkpoint stream item.
// Note, checkpoint::Checkpoint may contain either checkpoint data or summary.
type CheckpointStreamResult = Result<checkpoint::Checkpoint, Status>;

// Helper trait for getting checkpoint data and summaries,
// intended as an abstractoin for Arc<dyn RestStateReader>.
trait CheckpointOracle<T>
where
    T: Send + Sync + 'static,
    Self: Send + Sync + 'static,
{
    fn get_index(&self, item: &Arc<T>) -> u64;
    fn ser(&self, item: &Arc<T>) -> Result<Vec<u8>, bcs::Error>;
    fn get_item(&self, ix: u64) -> Option<Arc<T>>;
    fn get_latest(&self) -> Option<u64>;

    fn ser2(&self, item: &Arc<T>) -> CheckpointStreamResult {
        self.ser(item)
            .map(|data| checkpoint::Checkpoint {
                index: self.get_index(item),
                data,
            })
            .map_err(|e| Status::internal(format!("BCS serialization error: {e}")))
    }
}

#[derive(Clone)]
struct Oracle {
    state_reader: Arc<dyn RestStateReader>,
}

fn get_full_checkpoint_data(
    state_reader: &std::sync::Arc<dyn RestStateReader>,
    seq: u64,
) -> Option<CheckpointData> {
    let summary = state_reader.get_checkpoint_by_sequence_number(seq).ok()??;
    let contents = state_reader
        .get_checkpoint_contents_by_sequence_number(seq)
        .ok()??;
    state_reader.get_checkpoint_data(summary, contents).ok()
}

impl CheckpointOracle<GrpcCheckpointData> for Oracle {
    fn get_index(&self, item: &Arc<GrpcCheckpointData>) -> u64 {
        item.sequence_number()
    }
    fn ser(&self, item: &Arc<GrpcCheckpointData>) -> Result<Vec<u8>, bcs::Error> {
        bcs::to_bytes(&**item)
    }
    fn get_item(&self, ix: u64) -> Option<Arc<GrpcCheckpointData>> {
        get_full_checkpoint_data(&self.state_reader, ix)
            .map(GrpcCheckpointData::from)
            .map(Arc::new)
    }
    fn get_latest(&self) -> Option<u64> {
        self.state_reader
            .get_highest_synced_checkpoint()
            .ok()
            .map(|cp| *cp.sequence_number())
    }
}

impl CheckpointOracle<GrpcCertifiedCheckpointSummary> for Oracle {
    fn get_index(&self, item: &Arc<GrpcCertifiedCheckpointSummary>) -> u64 {
        item.sequence_number()
    }
    fn ser(&self, item: &Arc<GrpcCertifiedCheckpointSummary>) -> Result<Vec<u8>, bcs::Error> {
        bcs::to_bytes(&**item)
    }
    fn get_item(&self, ix: u64) -> Option<Arc<GrpcCertifiedCheckpointSummary>> {
        self.state_reader
            .get_checkpoint_by_sequence_number(ix)
            .ok()
            .flatten()
            .map(|v| GrpcCertifiedCheckpointSummary::from(CertifiedCheckpointSummary::from(v)))
            .map(Arc::new)
    }
    fn get_latest(&self) -> Option<u64> {
        self.state_reader
            .get_highest_synced_checkpoint()
            .ok()
            .map(|cp| *cp.sequence_number())
    }
}

fn create_checkpoint_stream<T, F>(
    oracle: F,
    tx: tokio::sync::broadcast::Sender<Arc<T>>,
    start: Option<u64>,
    end: Option<u64>,
) -> impl futures::Stream<Item = CheckpointStreamResult> + Send
where
    T: Send + Sync + 'static,
    F: CheckpointOracle<T> + Clone + Send + Sync + 'static,
{
    stream! {
        let mut rx = tx.subscribe();
        let start_idx = match (start, end) {
            (None, None) => oracle.get_latest().unwrap_or(0),
            _ => start.unwrap_or(0),
        };

        let mut current = start_idx;
        let mut last_sent = None;

        // Special case: only end_index provided
        if start.is_none() && end.is_some() {
            let idx = end.unwrap();
            if let Some(item) = oracle.get_item(idx) {
                yield oracle.ser2(&item);
                return;
            }
            // Wait for it on broadcast
            loop {
                match rx.recv().await {
                    Ok(item) if oracle.get_index(&item) == idx => {
                        yield oracle.ser2(&item);
                        return;
                    }
                    Ok(_) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => return,
                }
            }
        }

        // Historical phase
        let latest = oracle.get_latest().unwrap_or(current);
        let stop_at = end.map(|e| latest.min(e)).unwrap_or(latest);

        while current <= stop_at {
            if let Some(item) = oracle.get_item(current) {
                let result = oracle.ser2(&item);
                if let Ok(c) = &result {
                    last_sent = Some(c.index);
                    current += 1;
                }
                yield result;
            } else {
                last_sent = Some(current.saturating_sub(1));
                break;
            }
        }

        if current > stop_at {
            last_sent = Some(stop_at);
        }

        // Live phase
        let mut last_sent_idx = last_sent.unwrap_or(current.saturating_sub(1));

        loop {
            // Try DB first
            if let Some(item) = oracle.get_item(last_sent_idx + 1) {
                let result = oracle.ser2(&item);
                if let Ok(c) = &result {
                    tracing::info!("[GAP FILL] Sent missing checkpoint {} from DB (prev last_sent: {})", c.index, last_sent_idx);
                    last_sent_idx = c.index;
                } else {
                    tracing::error!("[GAP FILL ERROR] BCS serialization error for checkpoint {}: {}", last_sent_idx + 1, result.as_ref().unwrap_err());
                }
                yield result;
                continue;
            }

            // Wait for broadcast
            match rx.recv().await {
                Ok(item) => {
                    let idx = oracle.get_index(&item);
                    if let Some(end_idx) = end {
                        if idx > end_idx { return; }
                    }
                    if idx == last_sent_idx + 1 {
                        let result = oracle.ser2(&item);
                        if result.is_ok() {
                            last_sent_idx = idx;
                        }
                        yield result;
                    }
                    // Skip duplicates/out-of-order, continue for gaps
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    let latest = oracle.get_latest().unwrap_or(last_sent_idx);
                    let stop_at = end.map(|e| latest.min(e)).unwrap_or(latest);
                    while last_sent_idx < stop_at {
                        if let Some(item) = oracle.get_item(last_sent_idx + 1) {
                            let result = oracle.ser2(&item);
                            if let Ok(c) = &result {
                                tracing::info!("[GAP FILL] Sent missing checkpoint {} from DB (prev last_sent: {})", c.index, last_sent_idx);
                                last_sent_idx = c.index;
                            }
                            yield result;
                        } else {
                            break;
                        }
                    }
                }
                Err(_) => return,
            }
        }
    }
}

fn create_checkpoint_stream2<T, F>(
    oracle: F,
    tx: tokio::sync::broadcast::Sender<Arc<T>>,
    start: Option<u64>,
    end: Option<u64>,
) -> impl futures::Stream<Item = CheckpointStreamResult> + Send
where
    T: Send + Sync + 'static,
    F: CheckpointOracle<T> + Clone + Send + Sync + 'static,
{
    async_stream::try_stream! {
        let mut rx = tx.subscribe();
        let mut latest = oracle.get_latest().unwrap_or(0);
        let (mut start, end) = match (start, end) {
            (None, None) => (latest, u64::MAX),
            (None, Some(end)) => (end, end),
            (Some(start), None) => (start, u64::MAX),
            (Some(start), Some(end)) => (start, end),
        };
        let mut cached = None;

        while start <= end {
            // try fetching historical data from the DB first
            if start <= latest {
                if let Some(item) = oracle.get_item(start) {
                    // TODO: add backfill tracing messages
                    yield oracle.ser2(&item)?;
                    if start == end {
                        break;
                    }
                    start += 1;
                    continue;
                } else {
                    // report error the the stream and break
                    Err(Status::internal(format!("Historical checkpoint data missing/pruned: index={start} latest={latest}.")))?;
                    break;
                }
            }
            // latest < start, live phase
            if let Some(item) = cached.take() {
                // already have something in cache
                let idx = oracle.get_index(&item);
                if start == idx {
                    yield oracle.ser2(&item)?;
                    if start == end {
                        break;
                    }
                    start += 1;
                } else if start < idx {
                    cached = Some(item);
                }
            }
            // wait for broadcast
            match rx.recv().await {
                Ok(item) => {
                    let idx = oracle.get_index(&item);
                    if start == idx {
                        yield oracle.ser2(&item)?;
                        if start == end {
                            break;
                        }
                        start += 1;
                        continue;
                    } else if start < idx {
                        // the item is too fresh, need to fill the gap from history DB
                        cached = Some(item);
                    } // else item is too old, just drop it and continue
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
            latest = oracle.get_latest().unwrap_or(start);
        }
    }
}

impl CheckpointGrpcService {
    fn stream_checkpoint_data(
        &self,
        start: Option<u64>,
        end: Option<u64>,
    ) -> impl futures::Stream<Item = CheckpointStreamResult> + Send {
        let state_reader = self.state_reader.clone();
        let oracle = Oracle { state_reader };
        create_checkpoint_stream2(oracle, self.grpc_checkpoint_data_tx.clone(), start, end)
    }

    fn stream_checkpoint_summary(
        &self,
        start: Option<u64>,
        end: Option<u64>,
    ) -> impl futures::Stream<Item = CheckpointStreamResult> + Send {
        let state_reader = self.state_reader.clone();
        let oracle = Oracle { state_reader };
        create_checkpoint_stream2(oracle, self.grpc_checkpoint_summary_tx.clone(), start, end)
    }
}

#[tonic::async_trait]
impl CheckpointService for CheckpointGrpcService {
    type StreamCheckpointsStream =
        Pin<Box<dyn futures::Stream<Item = Result<checkpoint::Checkpoint, Status>> + Send>>;

    async fn stream_checkpoints(
        &self,
        request: Request<checkpoint::StreamRequest>,
    ) -> Result<Response<Self::StreamCheckpointsStream>, Status> {
        let req = request.into_inner();
        let start = req.start_index;
        let end = req.end_index;
        let full = req.full;

        let stream: Self::StreamCheckpointsStream = if full.unwrap_or(false) {
            Box::pin(self.stream_checkpoint_data(start, end))
        } else {
            Box::pin(self.stream_checkpoint_summary(start, end))
        };
        Ok(Response::new(stream))
    }

    async fn get_epoch_first_checkpoint_sequence_number(
        &self,
        request: Request<checkpoint::EpochRequest>,
    ) -> Result<Response<checkpoint::CheckpointSequenceNumberResponse>, Status> {
        let epoch = request.into_inner().epoch;
        tracing::info!(
            "get_epoch_first_checkpoint_sequence_number called for epoch {}",
            epoch
        );

        let latest_seq = match self.state_reader.get_highest_synced_checkpoint() {
            Ok(cp) => *cp.sequence_number(),
            Err(_) => {
                tracing::info!("No checkpoints found in the system for epoch {}", epoch);
                return Ok(Response::new(
                    checkpoint::CheckpointSequenceNumberResponse { sequence_number: 0 },
                ));
            }
        };

        if let Ok(Some(latest_summary)) = self
            .state_reader
            .get_checkpoint_by_sequence_number(latest_seq)
        {
            if epoch > latest_summary.epoch {
                tracing::info!(
                    "Requested epoch {} > latest epoch {}",
                    epoch,
                    latest_summary.epoch
                );
                return Ok(Response::new(
                    checkpoint::CheckpointSequenceNumberResponse { sequence_number: 0 },
                ));
            }

            if latest_summary.epoch == epoch {
                return Ok(Response::new(
                    checkpoint::CheckpointSequenceNumberResponse {
                        sequence_number: self.find_epoch_start_backwards(epoch, latest_seq).await,
                    },
                ));
            }
        }

        let found_seq = self.binary_search_epoch_start(epoch, latest_seq).await;

        tracing::info!(
            "Found first checkpoint for epoch {}: seq={}",
            epoch,
            found_seq
        );

        Ok(Response::new(
            checkpoint::CheckpointSequenceNumberResponse {
                sequence_number: found_seq,
            },
        ))
    }
}

impl CheckpointGrpcService {
    async fn binary_search_epoch_start(&self, target_epoch: u64, latest_seq: u64) -> u64 {
        let mut left = 0u64;
        let mut right = latest_seq;
        let epoch_start = 0u64;

        while left <= right {
            let mid = left + (right - left) / 2;

            match self.state_reader.get_checkpoint_by_sequence_number(mid) {
                Ok(Some(summary)) => {
                    if summary.epoch == target_epoch {
                        return self.find_epoch_start_backwards(target_epoch, mid).await;
                    } else if summary.epoch < target_epoch {
                        left = mid + 1;
                    } else {
                        if mid == 0 {
                            break;
                        }
                        right = mid - 1;
                    }
                }
                _ => {
                    if mid == 0 {
                        break;
                    }
                    right = mid - 1;
                }
            }
        }

        epoch_start
    }

    async fn find_epoch_start_backwards(&self, target_epoch: u64, start_seq: u64) -> u64 {
        tracing::debug!(
            "Finding epoch {} start, searching backwards from seq {}",
            target_epoch,
            start_seq
        );

        let mut current_seq = start_seq;
        let mut first_seq = start_seq;

        loop {
            match self
                .state_reader
                .get_checkpoint_by_sequence_number(current_seq)
            {
                Ok(Some(summary)) => {
                    tracing::debug!(
                        "Checkpoint {} has epoch {}, target epoch {}",
                        current_seq,
                        summary.epoch,
                        target_epoch
                    );

                    if summary.epoch == target_epoch {
                        first_seq = current_seq;
                        if current_seq == 0 {
                            tracing::debug!(
                                "Reached checkpoint 0, stopping search. First seq for epoch {}: {}",
                                target_epoch,
                                first_seq
                            );
                            break;
                        }
                        current_seq = current_seq - 1;
                    } else {
                        tracing::debug!(
                            "Found different epoch {} at seq {}, stopping search. First seq for epoch {}: {}",
                            summary.epoch,
                            current_seq,
                            target_epoch,
                            first_seq
                        );
                        break;
                    }
                }
                _ => {
                    tracing::debug!("No checkpoint found at seq {}", current_seq);
                    if current_seq == 0 {
                        break;
                    }
                    current_seq = current_seq - 1;
                }
            }
        }

        tracing::debug!(
            "Final result: first checkpoint of epoch {} is {}",
            target_epoch,
            first_seq
        );
        first_seq
    }
}
