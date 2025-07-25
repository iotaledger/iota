// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{pin::Pin, sync::Arc};

use iota_grpc_types::{
    CertifiedCheckpointSummary as GrpcCertifiedCheckpointSummary,
    CheckpointData as GrpcCheckpointData,
};
use iota_types::storage::RestStateReader;
use tonic::{Request, Response, Status};
use tracing::{debug, info};

use crate::{
    checkpoint::{
        CheckpointSequenceNumberResponse, CheckpointStreamRequest, EpochRequest,
        checkpoint_service_server::CheckpointService,
    },
    types::*,
};

pub struct CheckpointGrpcService {
    pub reader: Reader,
    pub checkpoint_summary_tx: tokio::sync::broadcast::Sender<Arc<GrpcCertifiedCheckpointSummary>>,
    pub checkpoint_data_tx: tokio::sync::broadcast::Sender<Arc<GrpcCheckpointData>>,
}

impl CheckpointGrpcService {
    pub fn new(
        state_reader: Arc<dyn RestStateReader>,
        checkpoint_summary_tx: tokio::sync::broadcast::Sender<Arc<GrpcCertifiedCheckpointSummary>>,
        checkpoint_data_tx: tokio::sync::broadcast::Sender<Arc<GrpcCheckpointData>>,
    ) -> Self {
        Self {
            reader: Reader { state_reader },
            checkpoint_summary_tx,
            checkpoint_data_tx,
        }
    }
}

impl CheckpointGrpcService {
    fn stream_checkpoint_data(
        &self,
        start_sequence_number: Option<u64>,
        end_sequence_number: Option<u64>,
    ) -> impl futures::Stream<Item = CheckpointStreamResult> + Send {
        self.reader.create_checkpoint_stream(
            self.checkpoint_data_tx.subscribe(),
            start_sequence_number,
            end_sequence_number,
            true,
        )
    }

    fn stream_checkpoint_summary(
        &self,
        start_sequence_number: Option<u64>,
        end_sequence_number: Option<u64>,
    ) -> impl futures::Stream<Item = CheckpointStreamResult> + Send {
        self.reader.create_checkpoint_stream(
            self.checkpoint_summary_tx.subscribe(),
            start_sequence_number,
            end_sequence_number,
            false,
        )
    }
}

#[tonic::async_trait]
impl CheckpointService for CheckpointGrpcService {
    type StreamCheckpointsStream =
        Pin<Box<dyn futures::Stream<Item = Result<crate::checkpoint::Checkpoint, Status>> + Send>>;

    async fn stream_checkpoints(
        &self,
        request: Request<CheckpointStreamRequest>,
    ) -> Result<Response<Self::StreamCheckpointsStream>, Status> {
        let req = request.into_inner();
        let start_sequence_number = req.start_sequence_number;
        let end_sequence_number = req.end_sequence_number;
        let full = req.full;

        let stream: Self::StreamCheckpointsStream = if full {
            Box::pin(self.stream_checkpoint_data(start_sequence_number, end_sequence_number))
        } else {
            Box::pin(self.stream_checkpoint_summary(start_sequence_number, end_sequence_number))
        };
        Ok(Response::new(stream))
    }

    async fn get_epoch_first_checkpoint_sequence_number(
        &self,
        request: Request<EpochRequest>,
    ) -> Result<Response<CheckpointSequenceNumberResponse>, Status> {
        let epoch = request.into_inner().epoch;
        debug!(
            "get_epoch_first_checkpoint_sequence_number called for epoch {}",
            epoch
        );

        let sequence_number = if epoch == 0 {
            // Genesis epoch starts at checkpoint 0
            0
        } else {
            // Get the last checkpoint of the previous epoch
            match self
                .reader
                .state_reader
                .get_epoch_last_checkpoint(epoch - 1)
            {
                Ok(Some(last_checkpoint)) => {
                    // First checkpoint of current epoch is the next one
                    *last_checkpoint.sequence_number() + 1
                }
                Ok(None) => {
                    return Err(Status::not_found(format!(
                        "No checkpoints found for previous epoch {}",
                        epoch - 1
                    )));
                }
                Err(e) => {
                    return Err(Status::internal(format!(
                        "Failed to get last checkpoint for epoch {}: {}",
                        epoch - 1,
                        e
                    )));
                }
            }
        };

        info!(
            "First checkpoint for epoch {}: seq={}",
            epoch, sequence_number
        );

        Ok(Response::new(CheckpointSequenceNumberResponse {
            sequence_number,
        }))
    }
}
