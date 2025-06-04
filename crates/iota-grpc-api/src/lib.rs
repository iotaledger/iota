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

// Trait to extend Arc<dyn RestStateReader> with get_full_checkpoint_data
pub trait FullCheckpointDataExt {
    fn get_full_checkpoint_data(&self, seq: u64) -> Option<CheckpointData>;
}

impl FullCheckpointDataExt for std::sync::Arc<dyn RestStateReader> {
    fn get_full_checkpoint_data(&self, seq: u64) -> Option<CheckpointData> {
        let summary = self.get_checkpoint_by_sequence_number(seq).ok()??;
        let contents = self
            .get_checkpoint_contents_by_sequence_number(seq)
            .ok()??;
        self.get_checkpoint_data(summary, contents).ok()
    }
}

// Add a generic helper function for streaming checkpoints
fn stream_checkpoints_helper<T, FGetIndex, FSer, FGetItem>(
    tx: tokio::sync::broadcast::Sender<Arc<T>>,
    get_index: FGetIndex,
    ser: FSer,
    get_item: FGetItem,
    start: Option<u64>,
    end: Option<u64>,
    latest: Option<u64>,
) -> Result<
    Pin<Box<dyn futures::Stream<Item = Result<checkpoint::Checkpoint, Status>> + Send>>,
    Status,
>
where
    T: Send + Sync + 'static,
    FGetIndex: Fn(&Arc<T>) -> u64 + Send + Sync + 'static,
    FSer: Fn(&Arc<T>) -> Result<Vec<u8>, bcs::Error> + Send + Sync + 'static,
    FGetItem: Fn(u64) -> Option<Arc<T>> + Send + Sync + 'static,
{
    use std::collections::VecDeque;

    use futures::stream::unfold;
    let mut last_sent: Option<u64> = None;
    let stream_range = (start, end);
    let mut items = VecDeque::new();
    let start_idx = start.unwrap_or(0);
    let end_idx = end.or(latest);

    // Special logic: if only end_index is provided, only stream that checkpoint
    if start.is_none() && end.is_some() {
        let idx = end.unwrap();
        if let Some(item) = get_item(idx) {
            let data = ser(&item)
                .map_err(|e| Status::internal(format!("BCS serialization error: {e}")))?;
            items.push_back(Ok(checkpoint::Checkpoint {
                index: get_index(&item),
                data,
            }));
            last_sent = Some(idx);
        } else {
            let rx = tx.subscribe();
            let stream = unfold(
                (rx, get_index, ser, idx),
                |(mut rx, get_index, ser, idx)| async move {
                    loop {
                        match rx.recv().await {
                            Ok(item) => {
                                if get_index(&item) == idx {
                                    let data = ser(&item)
                                        .map_err(|e| {
                                            Status::internal(format!(
                                                "BCS serialization error: {e}"
                                            ))
                                        })
                                        .unwrap();
                                    return Some((
                                        Ok(checkpoint::Checkpoint { index: idx, data }),
                                        (rx, get_index, ser, idx),
                                    ));
                                }
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(_) => return None,
                        }
                    }
                },
            );
            return Ok(Box::pin(stream));
        }
    } else if start.is_none() && end.is_none() {
        if let Some(latest_idx) = latest {
            if let Some(item) = get_item(latest_idx) {
                let data = ser(&item)
                    .map_err(|e| Status::internal(format!("BCS serialization error: {e}")))?;
                items.push_back(Ok(checkpoint::Checkpoint {
                    index: get_index(&item),
                    data,
                }));
                last_sent = Some(latest_idx);
            }
        }
    } else if let Some(latest_idx) = latest {
        let hist_end = match end_idx {
            Some(e) if e <= latest_idx => Some(e),
            _ => Some(latest_idx),
        };
        if start_idx <= latest_idx {
            for idx in start_idx..=hist_end.unwrap() {
                if let Some(item) = get_item(idx) {
                    let data = ser(&item)
                        .map_err(|e| Status::internal(format!("BCS serialization error: {e}")))?;
                    items.push_back(Ok(checkpoint::Checkpoint {
                        index: get_index(&item),
                        data,
                    }));
                    last_sent = Some(idx);
                }
            }
        }
    }
    let rx = tx.subscribe();
    let stream = unfold(
        (
            items,
            last_sent,
            stream_range,
            rx,
            get_item,
            ser,
            get_index,
            end_idx,
        ),
        |(mut items, last_sent, stream_range, mut rx, get_item, ser, get_index, end_idx)| async move {
            if let Some(item) = items.pop_front() {
                return Some((
                    item,
                    (
                        items,
                        last_sent,
                        stream_range,
                        rx,
                        get_item,
                        ser,
                        get_index,
                        end_idx,
                    ),
                ));
            }
            loop {
                match rx.recv().await {
                    Ok(item) => {
                        let idx = get_index(&item);
                        if let Some(start) = stream_range.0 {
                            if idx < start {
                                continue;
                            }
                        }
                        if let Some(end) = stream_range.1 {
                            if idx > end {
                                return None;
                            }
                        }
                        if let Some(last) = last_sent {
                            if idx > last + 1 {
                                for missed in (last + 1)..idx {
                                    if let Some(missed_item) = get_item(missed) {
                                        let data = ser(&missed_item)
                                            .map_err(|e| {
                                                Status::internal(format!(
                                                    "BCS serialization error: {e}"
                                                ))
                                            })
                                            .unwrap();
                                        return Some((
                                            Ok(checkpoint::Checkpoint {
                                                index: get_index(&missed_item),
                                                data,
                                            }),
                                            (
                                                items,
                                                Some(missed),
                                                stream_range,
                                                rx,
                                                get_item,
                                                ser,
                                                get_index,
                                                end_idx,
                                            ),
                                        ));
                                    }
                                }
                            }
                        }
                        let data = ser(&item)
                            .map_err(|e| Status::internal(format!("BCS serialization error: {e}")))
                            .unwrap();
                        let done = if let Some(end) = end_idx {
                            idx >= end
                        } else {
                            false
                        };
                        let next_last = Some(idx);
                        if done {
                            return Some((
                                Ok(checkpoint::Checkpoint { index: idx, data }),
                                (
                                    VecDeque::new(),
                                    next_last,
                                    stream_range,
                                    rx,
                                    get_item,
                                    ser,
                                    get_index,
                                    end_idx,
                                ),
                            ));
                        }
                        return Some((
                            Ok(checkpoint::Checkpoint { index: idx, data }),
                            (
                                VecDeque::new(),
                                next_last,
                                stream_range,
                                rx,
                                get_item,
                                ser,
                                get_index,
                                end_idx,
                            ),
                        ));
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => return None,
                }
            }
        },
    );
    Ok(Box::pin(stream))
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

        if full.unwrap_or(false) {
            let tx = self.grpc_checkpoint_data_tx.clone();
            let state_reader = self.state_reader.clone();
            let get_index = |d: &Arc<CheckpointData>| d.checkpoint_summary.sequence_number;
            let ser = |d: &Arc<CheckpointData>| bcs::to_bytes(&**d);
            let get_item = move |idx| state_reader.get_full_checkpoint_data(idx).map(Arc::new);
            let latest = self
                .state_reader
                .get_latest_checkpoint_sequence_number()
                .ok();
            let stream =
                stream_checkpoints_helper(tx, get_index, ser, get_item, start, end, latest)?;
            Ok(Response::new(stream))
        } else {
            let tx = self.grpc_checkpoint_summary_tx.clone();
            let state_reader = self.state_reader.clone();
            let get_index = |d: &Arc<CertifiedCheckpointSummary>| d.data().sequence_number;
            let ser = |d: &Arc<CertifiedCheckpointSummary>| bcs::to_bytes(&d.data());
            let get_item = move |idx| {
                state_reader
                    .get_checkpoint_by_sequence_number(idx)
                    .ok()
                    .flatten()
                    .map(|v| Arc::new(v.into()))
            };
            let latest = self
                .state_reader
                .get_latest_checkpoint_sequence_number()
                .ok();
            let stream =
                stream_checkpoints_helper(tx, get_index, ser, get_item, start, end, latest)?;
            Ok(Response::new(stream))
        }
    }

    async fn get_epoch_first_checkpoint_sequence_number(
        &self,
        request: Request<checkpoint::EpochRequest>,
    ) -> Result<Response<checkpoint::CheckpointSequenceNumberResponse>, Status> {
        let epoch = request.into_inner().epoch;
        println!(
            "[gRPC DEBUG] get_epoch_first_checkpoint_sequence_number called for epoch {}",
            epoch
        );
        // Iterate over checkpoints in ascending order to find the first in the
        // requested epoch
        let mut iter =
            stream_checkpoints_public(self.state_reader.clone(), Direction::Ascending, 0);
        let mut found = 0u64;
        while let Some(Ok((summary, _))) = iter.next() {
            println!(
                "[gRPC DEBUG] Inspecting checkpoint: seq={}, epoch={}",
                summary.sequence_number, summary.epoch
            );
            if summary.epoch == epoch {
                found = summary.sequence_number;
                println!(
                    "[gRPC DEBUG] Found first checkpoint for epoch {}: seq={}",
                    epoch, found
                );
                break;
            }
        }
        if found == 0 {
            println!("[gRPC DEBUG] No checkpoint found for epoch {}", epoch);
        }
        Ok(Response::new(
            checkpoint::CheckpointSequenceNumberResponse {
                sequence_number: found,
            },
        ))
    }
}
