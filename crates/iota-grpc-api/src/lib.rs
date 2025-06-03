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
use futures::{StreamExt as FuturesStreamExt, stream};
// Helper to fetch full CheckpointData from a RestStateReader
use iota_types::full_checkpoint_content::CheckpointData;
use iota_types::messages_checkpoint::CertifiedCheckpointSummary;
use tokio_stream::wrappers::BroadcastStream;

pub struct CheckpointGrpcService {
    pub state_reader: Arc<dyn RestStateReader>,
    pub checkpoint_summary_tx: tokio::sync::broadcast::Sender<Arc<CertifiedCheckpointSummary>>,
    pub buffer:
        Arc<tokio::sync::Mutex<std::collections::VecDeque<Arc<CertifiedCheckpointSummary>>>>,
}

impl CheckpointGrpcService {
    pub fn new(
        state_reader: Arc<dyn RestStateReader>,
        checkpoint_summary_tx: tokio::sync::broadcast::Sender<Arc<CertifiedCheckpointSummary>>,
        buffer: Arc<
            tokio::sync::Mutex<std::collections::VecDeque<Arc<CertifiedCheckpointSummary>>>,
        >,
    ) -> Self {
        Self {
            state_reader,
            checkpoint_summary_tx,
            buffer,
        }
    }
}

// Trait to extend Arc<dyn RestStateReader> with get_full_checkpoint_data
pub trait FullCheckpointDataExt {
    fn get_full_checkpoint_data(
        &self,
        seq: u64,
    ) -> Option<iota_types::full_checkpoint_content::CheckpointData>;
}

impl FullCheckpointDataExt for std::sync::Arc<dyn RestStateReader> {
    fn get_full_checkpoint_data(
        &self,
        seq: u64,
    ) -> Option<iota_types::full_checkpoint_content::CheckpointData> {
        let summary = self.get_checkpoint_by_sequence_number(seq).ok()??;
        let contents = self
            .get_checkpoint_contents_by_sequence_number(seq)
            .ok()??;
        Some(iota_types::full_checkpoint_content::CheckpointData {
            checkpoint_summary: summary.into_inner(),
            checkpoint_contents: contents,
            transactions: vec![], // Fill if available
        })
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
        println!("[gRPC DEBUG] stream_checkpoints handler called");
        let req = request.into_inner();
        let start = req.start_index;
        let end = req.end_index;
        let full = req.full;

        // Case 1: Both start_index and end_index omitted
        if start.is_none() && end.is_none() {
            let buffer = self.buffer.clone();
            fn arc_ptr<T>(arc: &std::sync::Arc<T>) -> usize {
                arc.as_ref() as *const T as usize
            }
            println!(
                "[gRPC DEBUG] buffer Arc ptr (gRPC): 0x{:x}",
                arc_ptr(&buffer)
            );
            let rx = self.checkpoint_summary_tx.subscribe();
            println!("[gRPC DEBUG] Subscribed to broadcast channel");
            // 1. Lock and clone the buffer
            let buf = buffer.lock().await;
            println!("[gRPC DEBUG] Buffer length: {}", buf.len());
            let buffered: Vec<_> = buf.back().cloned().into_iter().collect();
            drop(buf);
            // 2. Create a stream that yields all buffered checkpoints first
            let buffered_stream = futures::stream::iter(buffered.into_iter().map(|summary| {
                let data = match bcs::to_bytes(&summary.data()) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        return Err(Status::internal(format!("BCS serialization error: {e}")));
                    }
                };
                let checkpoint_proto = checkpoint::Checkpoint {
                    index: summary.data().sequence_number,
                    data,
                };
                println!(
                    "Buffered checkpoint: seq={}, epoch={}",
                    summary.data().sequence_number,
                    summary.data().epoch
                );
                Ok(checkpoint_proto)
            }));
            // 3. Then stream new checkpoints as they arrive
            let live_stream = BroadcastStream::new(rx).filter_map(move |result| async move {
                match result {
                    Ok(summary) => {
                        println!(
                            "[gRPC DEBUG] Received new checkpoint from broadcast: seq={}, epoch={}",
                            summary.data().sequence_number,
                            summary.data().epoch
                        );
                        let data = match bcs::to_bytes(&*summary) {
                            Ok(bytes) => bytes,
                            Err(e) => {
                                return Some(Err(Status::internal(format!(
                                    "BCS serialization error: {e}"
                                ))));
                            }
                        };
                        let checkpoint_proto = checkpoint::Checkpoint {
                            index: summary.data().sequence_number,
                            data,
                        };
                        println!(
                            "[gRPC DEBUG] Live checkpoint: seq={}, epoch={}",
                            summary.data().sequence_number,
                            summary.data().epoch
                        );
                        Some(Ok(checkpoint_proto))
                    }
                    Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(
                        skipped,
                    )) => {
                        println!(
                            "[gRPC DEBUG] Broadcast channel lagged, skipped {} messages",
                            skipped
                        );
                        None
                    }
                }
            });
            // 4. Chain the two streams together
            let full_stream = buffered_stream.chain(live_stream);
            println!("[gRPC DEBUG] Returning chained stream to client");
            return Ok(Response::new(Box::pin(full_stream)));
        }

        // Case 2: Only start_index provided
        if start.is_some() && end.is_none() {
            let start_index = start.unwrap();
            let buffer = self.buffer.clone();
            let mut rx = self.checkpoint_summary_tx.subscribe();
            let mut items = Vec::new();
            // 1. Subscribe first (already done above)
            // 2. Lock and read the buffer
            let mut buf = buffer.lock().await;
            // 3. Drain the channel into the buffer before unlocking
            while let Ok(summary) = rx.try_recv() {
                buf.push_back(summary.clone());
                if buf.len() > 100 {
                    buf.pop_front();
                }
            }
            // Send all buffered checkpoints with index >= start_index
            for summary in buf.iter() {
                if summary.data().sequence_number >= start_index {
                    let data = match bcs::to_bytes(&summary.data()) {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            items.push(Err(Status::internal(format!(
                                "BCS serialization error: {e}"
                            ))));
                            break;
                        }
                    };
                    let checkpoint_proto = checkpoint::Checkpoint {
                        index: summary.data().sequence_number,
                        data,
                    };
                    items.push(Ok(checkpoint_proto));
                }
            }
            drop(buf);
            // 4. Stream new checkpoints from the buffer as they arrive
            let live = Box::pin(BroadcastStream::new(rx).filter_map(move |result| {
                async move {
                    match result {
                        Ok(summary) => {
                            println!("[gRPC DEBUG] Received checkpoint from broadcast channel: seq={}, epoch={}", summary.data().sequence_number, summary.data().epoch);
                            let data = match bcs::to_bytes(&*summary) {
                                Ok(bytes) => bytes,
                                Err(e) => {
                                    return Some(Err(Status::internal(format!(
                                        "BCS serialization error: {e}"
                                    ))));
                                }
                            };
                            let checkpoint_proto = checkpoint::Checkpoint {
                                index: summary.data().sequence_number,
                                data,
                            };
                            Some(Ok(checkpoint_proto))
                        }
                        Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(
                            _,
                        )) => None,
                    }
                }
            }));
            let full_stream = futures::stream::iter(items).chain(live);
            return Ok(Response::new(Box::pin(full_stream)));
        }

        // Case 3: Only end_index provided
        if start.is_none() && end.is_some() {
            let end_index = end.unwrap();
            let full = req.full;
            let state_reader = self.state_reader.clone();
            let single = stream::unfold(false, move |done| {
                let state_reader = state_reader.clone();
                async move {
                    if done {
                        return None;
                    }
                    if full {
                        // Fetch full CheckpointData
                        let checkpoint_data = state_reader.get_full_checkpoint_data(end_index);
                        if let Some(data) = checkpoint_data {
                            let bytes = match bcs::to_bytes(&data) {
                                Ok(b) => b,
                                Err(e) => {
                                    return Some((
                                        Err(Status::internal(format!(
                                            "BCS serialization error: {e}"
                                        ))),
                                        true,
                                    ));
                                }
                            };
                            let checkpoint_proto = checkpoint::Checkpoint {
                                index: end_index,
                                data: bytes,
                            };
                            Some((Ok(checkpoint_proto), true))
                        } else {
                            None
                        }
                    } else {
                        // Existing summary logic
                        let summary = state_reader
                            .get_checkpoint_by_sequence_number(end_index)
                            .ok()
                            .flatten();
                        if let Some(summary) = summary {
                            let data = match bcs::to_bytes(&summary.data()) {
                                Ok(bytes) => bytes,
                                Err(e) => {
                                    return Some((
                                        Err(Status::internal(format!(
                                            "BCS serialization error: {e}"
                                        ))),
                                        true,
                                    ));
                                }
                            };
                            let checkpoint_proto = checkpoint::Checkpoint {
                                index: summary.data().sequence_number,
                                data,
                            };
                            Some((Ok(checkpoint_proto), true))
                        } else {
                            None
                        }
                    }
                }
            });
            return Ok(Response::new(Box::pin(single)));
        }

        // Case 4: Both start_index and end_index provided
        if let (Some(start_index), Some(end_index)) = (start, end) {
            let state_reader = self.state_reader.clone();
            let range = stream::unfold(start_index, move |current| {
                let state_reader = state_reader.clone();
                async move {
                    if current > end_index {
                        return None;
                    }
                    let summary = state_reader
                        .get_checkpoint_by_sequence_number(current)
                        .ok()
                        .flatten();
                    if let Some(summary) = summary {
                        let data = match bcs::to_bytes(&summary.data()) {
                            Ok(bytes) => bytes,
                            Err(e) => {
                                return Some((
                                    Err(Status::internal(format!("BCS serialization error: {e}"))),
                                    current + 1,
                                ));
                            }
                        };
                        let checkpoint_proto = checkpoint::Checkpoint {
                            index: summary.data().sequence_number,
                            data,
                        };
                        Some((Ok(checkpoint_proto), current + 1))
                    } else {
                        None
                    }
                }
            });
            return Ok(Response::new(Box::pin(range)));
        }

        // Fallback: empty stream
        Ok(Response::new(Box::pin(stream::empty())))
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
