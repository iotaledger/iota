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
use iota_types::{
    full_checkpoint_content::CheckpointData, messages_checkpoint::CertifiedCheckpointSummary,
};
use tokio_stream::wrappers::BroadcastStream;

pub struct CheckpointGrpcService {
    pub state_reader: Arc<dyn RestStateReader>,
    pub grpc_checkpoint_summary_tx: tokio::sync::broadcast::Sender<Arc<CertifiedCheckpointSummary>>,
    pub grpc_checkpoint_summary_buffer:
        Arc<tokio::sync::Mutex<std::collections::VecDeque<Arc<CertifiedCheckpointSummary>>>>,
    pub grpc_checkpoint_data_tx: tokio::sync::broadcast::Sender<Arc<CheckpointData>>,
    pub grpc_checkpoint_data_buffer:
        Arc<tokio::sync::Mutex<std::collections::VecDeque<Arc<CheckpointData>>>>,
}

impl CheckpointGrpcService {
    pub fn new(
        state_reader: Arc<dyn RestStateReader>,
        grpc_checkpoint_summary_tx: tokio::sync::broadcast::Sender<Arc<CertifiedCheckpointSummary>>,
        grpc_checkpoint_summary_buffer: Arc<
            tokio::sync::Mutex<std::collections::VecDeque<Arc<CertifiedCheckpointSummary>>>,
        >,
        grpc_checkpoint_data_tx: tokio::sync::broadcast::Sender<Arc<CheckpointData>>,
        grpc_checkpoint_data_buffer: Arc<
            tokio::sync::Mutex<std::collections::VecDeque<Arc<CheckpointData>>>,
        >,
    ) -> Self {
        Self {
            state_reader,
            grpc_checkpoint_summary_tx,
            grpc_checkpoint_summary_buffer,
            grpc_checkpoint_data_tx,
            grpc_checkpoint_data_buffer,
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

        // Helper: serialize and wrap as proto
        fn to_proto<T, F>(item: &T, index: u64, ser: F) -> Result<checkpoint::Checkpoint, Status>
        where
            F: Fn(&T) -> Result<Vec<u8>, bcs::Error>,
        {
            let data =
                ser(item).map_err(|e| Status::internal(format!("BCS serialization error: {e}")))?;
            Ok(checkpoint::Checkpoint { index, data })
        }

        // Helper: buffered + live stream for summary or full
        async fn buffered_and_live_stream<T, F, G>(
            buffer: Arc<tokio::sync::Mutex<std::collections::VecDeque<Arc<T>>>>,
            tx: tokio::sync::broadcast::Sender<Arc<T>>,
            start_index: Option<u64>,
            ser: F,
            get_index: G,
        ) -> impl futures::Stream<Item = Result<checkpoint::Checkpoint, Status>>
        where
            F: Fn(&T) -> Result<Vec<u8>, bcs::Error> + Send + Sync + 'static + Copy,
            G: Fn(&T) -> u64 + Send + Sync + 'static + Copy,
            T: Send + Sync + 'static,
        {
            let rx = tx.subscribe(); // subscribe first to avoid gap
            let start_index = start_index.unwrap_or(0);
            let buf = buffer.lock().await;
            let mut items = Vec::new();
            for item in buf.iter() {
                if get_index(item) >= start_index {
                    match to_proto(&**item, get_index(item), ser) {
                        Ok(proto) => items.push(Ok(proto)),
                        Err(e) => {
                            items.push(Err(e));
                            break;
                        }
                    }
                }
            }
            drop(buf);
            let live = Box::pin(BroadcastStream::new(rx).filter_map(move |result| {
                let ser = ser;
                let get_index = get_index;
                async move {
                    match result {
                        Ok(item) => {
                            if get_index(&item) >= start_index {
                                match to_proto(&*item, get_index(&item), ser) {
                                    Ok(proto) => Some(Ok(proto)),
                                    Err(e) => Some(Err(e)),
                                }
                            } else {
                                None
                            }
                        }
                        Err(_) => None,
                    }
                }
            }));
            futures::stream::iter(items).chain(live)
        }

        // Helper: single checkpoint (summary or full)
        fn single_checkpoint<T, F>(
            item: Option<T>,
            index: u64,
            ser: F,
        ) -> impl futures::Stream<Item = Result<checkpoint::Checkpoint, Status>>
        where
            F: Fn(&T) -> Result<Vec<u8>, bcs::Error> + Send + Sync + 'static + Copy,
            T: Send + Sync + 'static,
        {
            futures::stream::once(async move {
                match item {
                    Some(val) => to_proto(&val, index, ser),
                    None => Err(Status::not_found("Checkpoint not found")),
                }
            })
        }

        // Helper: range stream (summary or full)
        fn range_stream<T, F, H>(
            state_reader: Arc<dyn RestStateReader>,
            start: u64,
            end: u64,
            get_item: H,
            ser: F,
            get_index: fn(&T) -> u64,
        ) -> impl futures::Stream<Item = Result<checkpoint::Checkpoint, Status>>
        where
            F: Fn(&T) -> Result<Vec<u8>, bcs::Error> + Send + Sync + 'static + Copy,
            H: Fn(Arc<dyn RestStateReader>, u64) -> Option<T> + Send + Sync + 'static + Copy,
            T: Send + Sync + 'static,
        {
            stream::unfold(start, move |current| {
                let state_reader = state_reader.clone();
                async move {
                    if current > end {
                        return None;
                    }
                    let item = get_item(state_reader.clone(), current);
                    let result = match item {
                        Some(val) => to_proto(&val, get_index(&val), ser),
                        None => Err(Status::not_found("Checkpoint not found")),
                    };
                    Some((result, current + 1))
                }
            })
        }

        // Main logic
        let stream: Pin<
            Box<dyn futures::Stream<Item = Result<checkpoint::Checkpoint, Status>> + Send>,
        > = if full.unwrap_or(false) {
            // --- FULL (CheckpointData) ---
            match (start, end) {
                (None, None) => {
                    // Subscribe first to avoid gap
                    let tx = self.grpc_checkpoint_data_tx.clone();
                    let rx = tx.subscribe();
                    let buffer = self.grpc_checkpoint_data_buffer.clone();
                    let last = buffer.lock().await.back().cloned();
                    let initial: Pin<
                        Box<
                            dyn futures::Stream<Item = Result<checkpoint::Checkpoint, Status>>
                                + Send,
                        >,
                    > = match last {
                        Some(data) => Box::pin(futures::stream::once(async move {
                            to_proto(&*data, data.checkpoint_summary.sequence_number, |d| {
                                bcs::to_bytes(d)
                            })
                        })),
                        None => Box::pin(futures::stream::empty()),
                    };
                    let live = Box::pin(BroadcastStream::new(rx).filter_map(
                        move |result| async move {
                            match result {
                                Ok(data) => Some(to_proto(
                                    &*data,
                                    data.checkpoint_summary.sequence_number,
                                    |d| bcs::to_bytes(d),
                                )),
                                Err(_) => None,
                            }
                        },
                    ));
                    Box::pin(initial.chain(live))
                }
                (Some(start_index), None) => Box::pin(
                    buffered_and_live_stream(
                        self.grpc_checkpoint_data_buffer.clone(),
                        self.grpc_checkpoint_data_tx.clone(),
                        Some(start_index),
                        |d| bcs::to_bytes(d),
                        |d| d.checkpoint_summary.sequence_number,
                    )
                    .await,
                ),
                (None, Some(end_index)) => {
                    // Single full checkpoint
                    Box::pin(single_checkpoint(
                        self.state_reader.get_full_checkpoint_data(end_index),
                        end_index,
                        |d| bcs::to_bytes(d),
                    ))
                }
                (Some(start_index), Some(end_index)) => {
                    // Range of full checkpoints
                    Box::pin(range_stream(
                        self.state_reader.clone(),
                        start_index,
                        end_index,
                        |reader, idx| reader.get_full_checkpoint_data(idx),
                        |d| bcs::to_bytes(d),
                        |d| d.checkpoint_summary.sequence_number,
                    ))
                }
            }
        } else {
            // --- SUMMARY (CertifiedCheckpointSummary) ---
            match (start, end) {
                (None, None) => {
                    // Subscribe first to avoid gap
                    let tx = self.grpc_checkpoint_summary_tx.clone();
                    let rx = tx.subscribe();
                    let buffer = self.grpc_checkpoint_summary_buffer.clone();
                    let last = buffer.lock().await.back().cloned();
                    let initial: Pin<
                        Box<
                            dyn futures::Stream<Item = Result<checkpoint::Checkpoint, Status>>
                                + Send,
                        >,
                    > = match last {
                        Some(data) => Box::pin(futures::stream::once(async move {
                            to_proto(&*data, data.data().sequence_number, |d| {
                                bcs::to_bytes(&d.data())
                            })
                        })),
                        None => Box::pin(futures::stream::empty()),
                    };
                    let live = Box::pin(BroadcastStream::new(rx).filter_map(
                        move |result| async move {
                            match result {
                                Ok(data) => {
                                    Some(to_proto(&*data, data.data().sequence_number, |d| {
                                        bcs::to_bytes(&d.data())
                                    }))
                                }
                                Err(_) => None,
                            }
                        },
                    ));
                    Box::pin(initial.chain(live))
                }
                (Some(start_index), None) => Box::pin(
                    buffered_and_live_stream(
                        self.grpc_checkpoint_summary_buffer.clone(),
                        self.grpc_checkpoint_summary_tx.clone(),
                        Some(start_index),
                        |d| bcs::to_bytes(&d.data()),
                        |d| d.data().sequence_number,
                    )
                    .await,
                ),
                (None, Some(end_index)) => Box::pin(single_checkpoint(
                    self.state_reader
                        .get_checkpoint_by_sequence_number(end_index)
                        .ok()
                        .flatten(),
                    end_index,
                    |d| bcs::to_bytes(&d.data()),
                )),
                (Some(start_index), Some(end_index)) => Box::pin(range_stream(
                    self.state_reader.clone(),
                    start_index,
                    end_index,
                    |reader, idx| reader.get_checkpoint_by_sequence_number(idx).ok().flatten(),
                    |d| bcs::to_bytes(&d.data()),
                    |d| d.data().sequence_number,
                )),
            }
        };
        Ok(Response::new(stream))
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
