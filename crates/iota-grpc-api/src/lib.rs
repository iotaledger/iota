// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{pin::Pin, sync::Arc};

use futures::Stream;
use tonic::{Request, Response, Status};
use tracing::info;
pub mod checkpoint {
    tonic::include_proto!("iota.grpc");
}

use bcs;
use checkpoint::{Checkpoint, StreamRequest, checkpoint_service_server::CheckpointService};
// In this PoC we use the public function from iota-rest-api to stream checkpoints.
// In the real implementation we will move the stream_checkpoints_public function and the
// associated logics to this crate.
use iota_rest_api::{Direction, stream_checkpoints_public};
use iota_types::{messages_checkpoint::CheckpointSequenceNumber, storage::RestStateReader};
pub mod client;

pub struct CheckpointGrpcService {
    pub state_reader: Arc<dyn RestStateReader>,
}

type CheckpointStream = Pin<Box<dyn Stream<Item = Result<Checkpoint, Status>> + Send>>;

#[tonic::async_trait]
impl CheckpointService for CheckpointGrpcService {
    type StreamCheckpointsStream = CheckpointStream;

    async fn stream_checkpoints(
        &self,
        request: Request<StreamRequest>,
    ) -> Result<Response<Self::StreamCheckpointsStream>, Status> {
        let req = request.into_inner();
        info!(
            "stream_checkpoints called with start_index={:?}, end_index={:?}",
            req.start_index, req.end_index
        );

        // Get latest checkpoint index
        let latest = stream_checkpoints_public(
            self.state_reader.clone(),
            Direction::Descending,
            CheckpointSequenceNumber::MAX,
        )
        .next()
        .and_then(|res| res.ok().map(|(summary, _)| summary.sequence_number))
        .unwrap_or(0);
        info!("Latest checkpoint index in store: {}", latest);

        let (start, end) = match (req.start_index, req.end_index) {
            (Some(start), Some(end)) => (start, std::cmp::min(latest, end)),
            (Some(start), None) => (start, latest),
            (None, Some(end)) => (end, end),
            (None, None) => (0, latest),
        };
        info!("Streaming checkpoints from {} to {}", start, end);

        let checkpoints: Vec<_> =
            stream_checkpoints_public(self.state_reader.clone(), Direction::Ascending, start)
                .take_while(move |res| {
                    res.as_ref()
                        .map(|(summary, _)| summary.sequence_number <= end)
                        .unwrap_or(false)
                })
                .filter_map(|res| match res {
                    Ok((certified_summary, contents)) => {
                        info!(
                            "Streaming checkpoint: {}",
                            certified_summary.sequence_number
                        );
                        // Use the state_reader's get_checkpoint_data to get full CheckpointData
                        match self.state_reader.get_checkpoint_data(
                            iota_types::message_envelope::VerifiedEnvelope::new_unchecked(
                                certified_summary.clone(),
                            ),
                            contents.clone(),
                        ) {
                            Ok(checkpoint_data) => Some(Ok(Checkpoint {
                                index: certified_summary.sequence_number,
                                data: bcs::to_bytes(&checkpoint_data).unwrap(),
                            })),
                            Err(e) => {
                                info!("Error building checkpoint data: {:?}", e);
                                None
                            }
                        }
                    }
                    Err(e) => {
                        info!("Error streaming checkpoint: {:?}", e);
                        None
                    }
                })
                .collect();

        if checkpoints.is_empty() {
            info!("No checkpoints to stream!");
        }
        let stream = futures::stream::iter(checkpoints);
        Ok(Response::new(Box::pin(stream) as CheckpointStream))
    }
}
