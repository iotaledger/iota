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
type CheckpointStreamResult = Result<checkpoint::Checkpoint, Status>;
fn stream_checkpoints_helper<T, FGetIndex, FSer, FGetItem, FGetLatest>(
    tx: tokio::sync::broadcast::Sender<Arc<T>>,
    get_index: FGetIndex,
    ser: FSer,
    get_item: FGetItem,
    get_latest: FGetLatest,
    start: Option<u64>,
    end: Option<u64>,
) -> Result<Pin<Box<dyn futures::Stream<Item = CheckpointStreamResult> + Send>>, Status>
where
    T: Send + Sync + 'static,
    FGetIndex: Fn(&Arc<T>) -> u64 + Send + Sync + 'static,
    FSer: Fn(&Arc<T>) -> Result<Vec<u8>, bcs::Error> + Send + Sync + 'static,
    FGetItem: Fn(u64) -> Option<Arc<T>> + Send + Sync + 'static,
    FGetLatest: Fn() -> Option<u64> + Send + Sync + 'static,
{
    let latest_idx = get_latest();
    let start_idx = match (start, end) {
        (None, None) => latest_idx.unwrap_or(0),
        _ => start.unwrap_or(0),
    };
    let end_idx = end;

    let stream = unfold(
        (
            start_idx,
            tx.subscribe(),
            get_index,
            ser,
            get_item,
            get_latest,
            end_idx,
            false,
            None,
            start,
            end,
        ),
        |(
            current,
            mut rx,
            get_index,
            ser,
            get_item,
            get_latest,
            end_idx,
            mut in_live_stream,
            mut last_sent,
            start,
            end,
        )| async move {
            // Special case: only end_index is provided, stream only that checkpoint
            if start.is_none() && end.is_some() && !in_live_stream {
                let idx = end.unwrap();
                if let Some(item) = get_item(idx) {
                    let data = match ser(&item) {
                        Ok(d) => d,
                        Err(e) => {
                            return Some((
                                Err(Status::internal(format!("BCS serialization error: {e}"))),
                                (
                                    current,
                                    rx,
                                    get_index,
                                    ser,
                                    get_item,
                                    get_latest,
                                    end_idx,
                                    true,
                                    Some(idx),
                                    start,
                                    end,
                                ),
                            ));
                        }
                    };
                    return Some((
                        Ok(checkpoint::Checkpoint {
                            index: get_index(&item),
                            data,
                        }),
                        (
                            current,
                            rx,
                            get_index,
                            ser,
                            get_item,
                            get_latest,
                            end_idx,
                            true,
                            Some(idx),
                            start,
                            end,
                        ),
                    ));
                } else {
                    // Not found in storage, wait for it to appear on the broadcast channel
                    loop {
                        match rx.recv().await {
                            Ok(item) => {
                                if get_index(&item) == idx {
                                    let data = match ser(&item) {
                                        Ok(d) => d,
                                        Err(e) => {
                                            return Some((
                                                Err(Status::internal(format!(
                                                    "BCS serialization error: {e}"
                                                ))),
                                                (
                                                    current,
                                                    rx,
                                                    get_index,
                                                    ser,
                                                    get_item,
                                                    get_latest,
                                                    end_idx,
                                                    true,
                                                    Some(idx),
                                                    start,
                                                    end,
                                                ),
                                            ));
                                        }
                                    };
                                    return Some((
                                        Ok(checkpoint::Checkpoint {
                                            index: get_index(&item),
                                            data,
                                        }),
                                        (
                                            current,
                                            rx,
                                            get_index,
                                            ser,
                                            get_item,
                                            get_latest,
                                            end_idx,
                                            true,
                                            Some(idx),
                                            start,
                                            end,
                                        ),
                                    ));
                                }
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(_) => return None,
                        }
                    }
                }
            }
            if !in_live_stream {
                let latest = get_latest().unwrap_or(current);
                let mut stop_at = end_idx.unwrap_or(latest);
                if stop_at > latest {
                    stop_at = latest;
                }
                if current <= stop_at {
                    if let Some(item) = get_item(current) {
                        let data = match ser(&item) {
                            Ok(d) => d,
                            Err(e) => {
                                return Some((
                                    Err(Status::internal(format!("BCS serialization error: {e}"))),
                                    (
                                        current,
                                        rx,
                                        get_index,
                                        ser,
                                        get_item,
                                        get_latest,
                                        end_idx,
                                        in_live_stream,
                                        last_sent,
                                        start,
                                        end,
                                    ),
                                ));
                            }
                        };
                        let next_current = current + 1;
                        let is_last = next_current > stop_at;
                        let item_index = get_index(&item);
                        return Some((
                            Ok(checkpoint::Checkpoint {
                                index: item_index,
                                data,
                            }),
                            (
                                next_current,
                                rx,
                                get_index,
                                ser,
                                get_item,
                                get_latest,
                                end_idx,
                                is_last,
                                Some(item_index),
                                start,
                                end,
                            ),
                        ));
                    } else {
                        in_live_stream = true;
                        last_sent = Some(current.saturating_sub(1));
                    }
                } else {
                    in_live_stream = true;
                    last_sent = Some(stop_at);
                }
            }
            let last_sent = last_sent.unwrap_or(current.saturating_sub(1));
            loop {
                // If this is the special end_index only case and in_live_stream is true, end
                // the stream
                if start.is_none() && end.is_some() && in_live_stream {
                    return None;
                }
                // Always try to fill the next expected checkpoint from DB first
                if let Some(missing_item) = get_item(last_sent + 1) {
                    let data = match ser(&missing_item) {
                        Ok(d) => d,
                        Err(e) => {
                            println!(
                                "[GAP FILL ERROR] BCS serialization error for checkpoint {}: {}",
                                last_sent + 1,
                                e
                            );
                            return Some((
                                Err(Status::internal(format!("BCS serialization error: {e}"))),
                                (
                                    current,
                                    rx,
                                    get_index,
                                    ser,
                                    get_item,
                                    get_latest,
                                    end_idx,
                                    in_live_stream,
                                    Some(last_sent),
                                    start,
                                    end,
                                ),
                            ));
                        }
                    };
                    let item_index = get_index(&missing_item);
                    println!(
                        "[GAP FILL] Sent missing checkpoint {} from DB (prev last_sent: {})",
                        item_index, last_sent
                    );
                    return Some((
                        Ok(checkpoint::Checkpoint {
                            index: item_index,
                            data,
                        }),
                        (
                            current,
                            rx,
                            get_index,
                            ser,
                            get_item,
                            get_latest,
                            end_idx,
                            in_live_stream,
                            Some(item_index),
                            start,
                            end,
                        ),
                    ));
                }
                // If not found in DB, wait for broadcast
                match rx.recv().await {
                    Ok(item) => {
                        let idx = get_index(&item);
                        if let Some(end) = end_idx {
                            if idx > end {
                                return None;
                            }
                        }
                        if idx == last_sent + 1 {
                            let data = match ser(&item) {
                                Ok(d) => d,
                                Err(e) => {
                                    return Some((
                                        Err(Status::internal(format!(
                                            "BCS serialization error: {e}"
                                        ))),
                                        (
                                            current,
                                            rx,
                                            get_index,
                                            ser,
                                            get_item,
                                            get_latest,
                                            end_idx,
                                            in_live_stream,
                                            Some(last_sent),
                                            start,
                                            end,
                                        ),
                                    ));
                                }
                            };
                            let item_index = get_index(&item);
                            return Some((
                                Ok(checkpoint::Checkpoint {
                                    index: item_index,
                                    data,
                                }),
                                (
                                    current,
                                    rx,
                                    get_index,
                                    ser,
                                    get_item,
                                    get_latest,
                                    end_idx,
                                    in_live_stream,
                                    Some(item_index),
                                    start,
                                    end,
                                ),
                            ));
                        } else if idx > last_sent + 1 {
                            // If a gap is detected, continue the loop to try to fill from DB
                            continue;
                        } else {
                            // Duplicate or out-of-order, skip
                            continue;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        let latest = get_latest().unwrap_or(last_sent);
                        let mut stop_at = end_idx.unwrap_or(latest);
                        if stop_at > latest {
                            stop_at = latest;
                        }
                        while last_sent < stop_at {
                            let next = last_sent + 1;
                            if let Some(item) = get_item(next) {
                                let data = match ser(&item) {
                                    Ok(d) => d,
                                    Err(e) => {
                                        return Some((
                                            Err(Status::internal(format!(
                                                "BCS serialization error: {e}"
                                            ))),
                                            (
                                                current,
                                                rx,
                                                get_index,
                                                ser,
                                                get_item,
                                                get_latest,
                                                end_idx,
                                                in_live_stream,
                                                Some(last_sent),
                                                start,
                                                end,
                                            ),
                                        ));
                                    }
                                };
                                let item_index = get_index(&item);
                                println!(
                                    "[GAP FILL] Sent missing checkpoint {} from DB (prev last_sent: {})",
                                    item_index, last_sent
                                );
                                return Some((
                                    Ok(checkpoint::Checkpoint {
                                        index: item_index,
                                        data,
                                    }),
                                    (
                                        current,
                                        rx,
                                        get_index,
                                        ser,
                                        get_item,
                                        get_latest,
                                        end_idx,
                                        in_live_stream,
                                        Some(item_index),
                                        start,
                                        end,
                                    ),
                                ));
                            } else {
                                break;
                            }
                        }
                        continue;
                    }
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
            let stream = stream_checkpoints_helper(
                tx,
                get_index,
                ser,
                get_item,
                move || latest,
                start,
                end,
            )?;
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
            let stream = stream_checkpoints_helper(
                tx,
                get_index,
                ser,
                get_item,
                move || latest,
                start,
                end,
            )?;
            Ok(Response::new(stream))
        }
    }

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
            .get_latest_checkpoint_sequence_number()
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
