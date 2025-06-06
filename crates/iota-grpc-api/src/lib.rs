// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{pin::Pin, sync::Arc};

use tonic::{Request, Response, Status};
pub mod checkpoint {
    tonic::include_proto!("iota.grpc");
}

use checkpoint::checkpoint_service_server::CheckpointService;
// In this PoC we use the public function from iota-rest-api to stream checkpoints.
// In the real implementation we will move the stream_checkpoints_public function and the
// associated logics to this crate.
use iota_rest_api::{Direction, stream_checkpoints_public};
use iota_types::storage::RestStateReader;
pub mod client;
use bcs;
use futures::stream::unfold;
use iota_types::{
    full_checkpoint_content::CheckpointData, messages_checkpoint::CertifiedCheckpointSummary,
};

pub struct CheckpointGrpcService {
    pub state_reader: Arc<dyn RestStateReader>,
    pub grpc_checkpoint_summary_tx: tokio::sync::broadcast::Sender<Arc<CertifiedCheckpointSummary>>,
    pub grpc_checkpoint_data_tx: tokio::sync::broadcast::Sender<Arc<CheckpointData>>,
}

impl CheckpointGrpcService {
    pub fn new(
        state_reader: Arc<dyn RestStateReader>,
        grpc_checkpoint_summary_tx: tokio::sync::broadcast::Sender<Arc<CertifiedCheckpointSummary>>,
        grpc_checkpoint_data_tx: tokio::sync::broadcast::Sender<Arc<CheckpointData>>,
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
trait CheckpointOracle<T> where
    T: Send + Sync + 'static,
    Self: Send + Sync + 'static,
{
    fn get_index(&self, item: &Arc<T>) -> u64;
    fn ser(&self, item: &Arc<T>) -> Result<Vec<u8>, bcs::Error>;
    fn get_item(&self, ix: u64) -> Option<Arc<T>>;
    fn get_latest(&self) -> Option<u64>;

    fn ser2(&self, item: &Arc<T>) -> Result<checkpoint::Checkpoint, Status> {
        self.ser(item)
            .map(|data| checkpoint::Checkpoint {
                index: self.get_index(item),
                data,
            })
            .map_err(|e| 
                Status::internal(format!("BCS serialization error: {e}"))
            )
    }
}

//
struct CheckpointStreamerState<T> {
    rx: tokio::sync::broadcast::Receiver<Arc<T>>,
    start: Option<u64>,
    end: Option<u64>,

    current: u64,
    in_live_stream: bool,
    last_sent: Option<u64>,
}

impl<T: Send + Sync + 'static> CheckpointStreamerState<T> {
    fn stream<F: CheckpointOracle<T> + Clone + Send + Sync + 'static>(
        oracle: F,
        tx: &tokio::sync::broadcast::Sender<Arc<T>>,
        start: Option<u64>,
        end: Option<u64>,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = CheckpointStreamResult> + Send>>, Status> 
    {
        let latest_idx = oracle.get_latest();
        let start_idx = match (start, end) {
            (None, None) => latest_idx.unwrap_or(0),
            _ => start.unwrap_or(0),
        };

        let init = CheckpointStreamerState {
            rx: tx.subscribe(),
            start,
            end,

            current: start_idx,
            in_live_stream: false,
            last_sent: None,
        };

        let stream = unfold(init,
            move |mut state| {
            let end_idx = state.end.clone();
            let oracle = oracle.clone();
            async move {
                // Special case: only end_index is provided, stream only that checkpoint
                if state.start.is_none() && state.end.is_some() && !state.in_live_stream {
                    let idx = state.end.unwrap();
                    if let Some(item) = oracle.get_item(idx) {
                        state.in_live_stream = true;
                        state.last_sent = Some(idx);
                        return Some((oracle.ser2(&item), state));
                    } else {
                        // Not found in storage, wait for it to appear on the broadcast channel
                        loop {
                            match state.rx.recv().await {
                                Ok(item) => {
                                    if oracle.get_index(&item) == idx {
                                        state.in_live_stream = true;
                                        state.last_sent = Some(idx);
                                        return Some((oracle.ser2(&item), state));
                                    }
                                }
                                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                                Err(_) => return None,
                            }
                        }
                    }
                }
                if !state.in_live_stream {
                    let latest = oracle.get_latest().unwrap_or(state.current);
                    let stop_at = end_idx.map(|e| latest.min(e)).unwrap_or(latest);
                    if state.current <= stop_at {
                        if let Some(item) = oracle.get_item(state.current) {
                            let result = oracle.ser2(&item);
                            if let Ok(c) = result.as_ref() {
                                state.current += 1;
                                state.in_live_stream = state.current > stop_at;
                                state.last_sent = Some(c.index);
                            }
                            return Some((result, state));
                        } else {
                            state.in_live_stream = true;
                            state.last_sent = Some(state.current.saturating_sub(1));
                        }
                    } else {
                        state.in_live_stream = true;
                        state.last_sent = Some(stop_at);
                    }
                }
                let last_sent = state.last_sent.unwrap_or(state.current.saturating_sub(1));
                loop {
                    // If this is the special end_index only case and in_live_stream is true, end
                    // the stream
                    if start.is_none() && end.is_some() && state.in_live_stream {
                        return None;
                    }
                    // Always try to fill the next expected checkpoint from DB first
                    if let Some(missing_item) = oracle.get_item(last_sent + 1) {
                        let result = oracle.ser2(&missing_item);
                        match result.as_ref() {
                            Ok(c) => {
                                tracing::info!(
                                    "[GAP FILL] Sent missing checkpoint {} from DB (prev last_sent: {})",
                                    c.index, last_sent
                                );
                                state.last_sent = Some(c.index);
                            },
                            Err(e) => {
                                tracing::error!(
                                    "[GAP FILL ERROR] BCS serialization error for checkpoint {}: {}",
                                    last_sent + 1,
                                    e
                                );
                                state.last_sent = Some(last_sent);
                            },
                        }
                        return Some((result, state));
                    }
                    // If not found in DB, wait for broadcast
                    match state.rx.recv().await {
                        Ok(item) => {
                            let idx = oracle.get_index(&item);
                            if let Some(end) = end_idx {
                                if idx > end {
                                    return None;
                                }
                            }
                            if idx == last_sent + 1 {
                                let result = oracle.ser2(&item);
                                match result.as_ref() {
                                    Ok(c) => {
                                        state.last_sent = Some(c.index);
                                    },
                                    Err(_) => {
                                        state.last_sent = Some(last_sent);
                                    },
                                }
                                return Some((result, state));
                            } else if idx > last_sent + 1 {
                                // If a gap is detected, continue the loop to try to fill from DB
                                continue;
                            } else {
                                // Duplicate or out-of-order, skip
                                continue;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            let latest = oracle.get_latest().unwrap_or(last_sent);
                            let stop_at = end_idx.map(|e| latest.min(e)).unwrap_or(latest);
                            while last_sent < stop_at {
                                let next = last_sent + 1;
                                if let Some(item) = oracle.get_item(next) {
                                    let result = oracle.ser2(&item);
                                    match result.as_ref() {
                                        Ok(c) => {
                                            tracing::info!(
                                                "[GAP FILL] Sent missing checkpoint {} from DB (prev last_sent: {})",
                                                c.index, last_sent
                                            );
                                            state.last_sent = Some(c.index);
                                        },
                                        Err(_) => {
                                            state.last_sent = Some(last_sent);
                                        },
                                    }
                                    return Some((result, state));
                                } else {
                                    break;
                                }
                            }
                            continue;
                        }
                        Err(_) => return None,
                    }
                }
            }},
        );

        Ok(Box::pin(stream))
    }
}

#[derive(Clone)]
struct Oracle {
    state_reader: Arc<dyn RestStateReader>,
}
fn get_full_checkpoint_data(state_reader: &std::sync::Arc<dyn RestStateReader>, seq: u64) -> Option<CheckpointData> {
    let summary = state_reader
        .get_checkpoint_by_sequence_number(seq)
        .ok()??;
    let contents = state_reader
        .get_checkpoint_contents_by_sequence_number(seq)
        .ok()??;
    state_reader
        .get_checkpoint_data(summary, contents)
        .ok()
}
impl CheckpointOracle<CheckpointData> for Oracle {
    fn get_index(&self, item: &Arc<CheckpointData>) -> u64 {
        item.checkpoint_summary.sequence_number
    }
    fn ser(&self, item: &Arc<CheckpointData>) -> Result<Vec<u8>, bcs::Error> {
        bcs::to_bytes(&**item)
    }
    fn get_item(&self, ix: u64) -> Option<Arc<CheckpointData>> {
        get_full_checkpoint_data(&self.state_reader, ix)
            .map(Arc::new)
    }
    fn get_latest(&self) -> Option<u64> {
        self.state_reader
            .get_latest_checkpoint_sequence_number()
            .ok()
    }
}
impl CheckpointOracle<CertifiedCheckpointSummary> for Oracle {
    fn get_index(&self, item: &Arc<CertifiedCheckpointSummary>) -> u64 {
        item.data().sequence_number
    }
    fn ser(&self, item: &Arc<CertifiedCheckpointSummary>) -> Result<Vec<u8>, bcs::Error> {
        bcs::to_bytes(&item.data())
    }
    fn get_item(&self, ix: u64) -> Option<Arc<CertifiedCheckpointSummary>> {
        self.state_reader
            .get_checkpoint_by_sequence_number(ix)
            .ok()
            .flatten()
            .map(|v| Arc::new(v.into()))
    }
    fn get_latest(&self) -> Option<u64> {
        self.state_reader
            .get_latest_checkpoint_sequence_number()
            .ok()
    }
}

impl CheckpointGrpcService {
    fn stream_checkpoint_data(
        &self,
        start: Option<u64>,
        end: Option<u64>,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = CheckpointStreamResult> + Send>>, Status> {
        let state_reader = self.state_reader.clone();
        let oracle = Oracle { state_reader };
        CheckpointStreamerState::stream(oracle, &self.grpc_checkpoint_data_tx, start, end)
    }
    fn stream_checkpoint_summary(
        &self,
        start: Option<u64>,
        end: Option<u64>,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = CheckpointStreamResult> + Send>>, Status> {
        let state_reader = self.state_reader.clone();
        let oracle = Oracle { state_reader };
        CheckpointStreamerState::stream(oracle, &self.grpc_checkpoint_summary_tx, start, end)
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

        let stream = if full.unwrap_or(false) {
            self.stream_checkpoint_data(start, end)
        } else {
            self.stream_checkpoint_summary(start, end)
        };
        Ok(Response::new(stream?))
    }

    // TODO: remove this?
    async fn get_epoch_first_checkpoint_sequence_number(
        &self,
        request: Request<checkpoint::EpochRequest>,
    ) -> Result<Response<checkpoint::CheckpointSequenceNumberResponse>, Status> {
        let epoch = request.into_inner().epoch;
        println!(
            "[gRPC] get_epoch_first_checkpoint_sequence_number called for epoch {}",
            epoch
        );

        let latest_seq_opt = self
            .state_reader
            .get_latest_checkpoint_sequence_number() // TODO: use get_highest_synced_checkpoint?
            .ok();

        if latest_seq_opt.is_none() {
            println!(
                "[gRPC] No checkpoints found in the system for epoch {}.",
                epoch
            );
            return Ok(Response::new(
                checkpoint::CheckpointSequenceNumberResponse { sequence_number: 0 },
            ));
        }
        let latest_seq = latest_seq_opt.unwrap();

        // Optimization: if the requested epoch is higher than the epoch of the latest
        // checkpoint, it cannot exist yet.
        match self
            .state_reader
            .get_checkpoint_by_sequence_number(latest_seq)
        {
            Ok(Some(latest_summary_envelope)) => {
                if epoch > latest_summary_envelope.epoch {
                    println!(
                        "[gRPC] Requested epoch {} is greater than the latest known epoch {}.",
                        epoch, latest_summary_envelope.epoch
                    );
                    return Ok(Response::new(
                        checkpoint::CheckpointSequenceNumberResponse { sequence_number: 0 },
                    ));
                }
            }
            Ok(None) => {
                println!(
                    "[gRPC] Latest checkpoint (seq {}) not found in store, though sequence number was reported. Proceeding with scan.",
                    latest_seq
                );
            }
            Err(e) => {
                println!(
                    "[gRPC] Error fetching latest checkpoint summary (seq {}) for optimization: {:?}. Proceeding with scan.",
                    latest_seq, e
                );
            }
        }

        println!(
            "[gRPC] Searching for first checkpoint of epoch {} by scanning backwards from seq {}",
            epoch, latest_seq
        );

        let mut iter =
            stream_checkpoints_public(self.state_reader.clone(), Direction::Descending, latest_seq);
        let mut found_seq = 0u64;

        while let Some(Ok((summary, _))) = iter.next() {
            // summary is VerifiedEnvelope<CertifiedCheckpointSummary>
            println!(
                "[gRPC] Inspecting checkpoint (desc): seq={}, epoch={}",
                summary.sequence_number, summary.epoch
            );

            if summary.epoch == epoch {
                found_seq = summary.sequence_number; // Keep updating, last one will be the smallest seq for this epoch
            } else if summary.epoch < epoch {
                // We've scanned past the target epoch into earlier epochs.
                // If found_seq is non-zero, it means we found the target epoch, and found_seq
                // holds its first seq. If found_seq is zero, the target epoch
                // was not found (e.g., target 1, saw epochs 2 then 0).
                println!(
                    "[gRPC] Scanned past target epoch {}. Current epoch: {}. First seq for target (if any): {}",
                    epoch, summary.epoch, found_seq
                );
                break;
            }
            // If summary.epoch > epoch, continue scanning downwards.
            // found_seq will remain 0 until we hit the target_epoch, or will
            // hold the latest update.
        }

        if found_seq == 0 {
            println!(
                "[gRPC] No checkpoint found for epoch {} after backward scan.",
                epoch
            );
        } else {
            println!(
                "[gRPC] Found first checkpoint for epoch {}: seq={}",
                epoch, found_seq
            );
        }

        Ok(Response::new(
            checkpoint::CheckpointSequenceNumberResponse {
                sequence_number: found_seq,
            },
        ))
    }
}
