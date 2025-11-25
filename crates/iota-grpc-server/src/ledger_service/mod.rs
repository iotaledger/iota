// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod get_epoch;
mod get_objects;
mod get_service_info;

use std::{pin::Pin, sync::Arc};

use iota_grpc_types::v0::ledger_service::{self as grpc_ledger_service};
use iota_protocol_config::Chain;
use iota_types::digests::ChainIdentifier;
use tokio_util::sync::CancellationToken;
use tonic::{Request, Response, Status};

use crate::types::*;

pub struct LedgerGrpcService {
    pub reader: Arc<GrpcReader>,
    pub checkpoint_summary_broadcaster: GrpcCheckpointSummaryBroadcaster,
    pub checkpoint_data_broadcaster: GrpcCheckpointDataBroadcaster,
    pub cancellation_token: CancellationToken,
    pub chain_id: ChainIdentifier,
    pub chain: Chain,
    pub server_version: Option<String>,
}

impl LedgerGrpcService {
    pub fn new(
        reader: Arc<GrpcReader>,
        checkpoint_summary_broadcaster: GrpcCheckpointSummaryBroadcaster,
        checkpoint_data_broadcaster: GrpcCheckpointDataBroadcaster,
        cancellation_token: CancellationToken,
        chain_id: ChainIdentifier,
        server_version: Option<String>,
    ) -> Self {
        Self {
            reader,
            checkpoint_summary_broadcaster,
            checkpoint_data_broadcaster,
            cancellation_token,
            chain_id,
            chain: chain_id.chain(),
            server_version,
        }
    }
}

impl LedgerGrpcService {
    fn stream_checkpoint_data(
        &self,
        start_sequence_number: Option<u64>,
        end_sequence_number: Option<u64>,
    ) -> impl futures::Stream<Item = CheckpointStreamResult> + Send {
        let rx = self.checkpoint_data_broadcaster.subscribe();
        self.reader.create_checkpoint_data_stream(
            rx,
            start_sequence_number,
            end_sequence_number,
            self.cancellation_token.clone(),
        )
    }
}

#[tonic::async_trait]
impl grpc_ledger_service::ledger_service_server::LedgerService for LedgerGrpcService {
    type GetObjectsStream = Pin<Box<dyn futures::Stream<Item = ObjectsStreamResult> + Send>>;
    type GetTransactionsStream =
        Pin<Box<dyn futures::Stream<Item = TransactionsStreamResult> + Send>>;
    type GetCheckpointDataStream =
        Pin<Box<dyn futures::Stream<Item = CheckpointStreamResult> + Send>>;
    type StreamCheckpointDataStream =
        Pin<Box<dyn futures::Stream<Item = CheckpointStreamResult> + Send>>;

    /// Query the service for general information about its current state.
    async fn get_service_info(
        &self,
        request: tonic::Request<grpc_ledger_service::GetServiceInfoRequest>,
    ) -> std::result::Result<
        tonic::Response<grpc_ledger_service::GetServiceInfoResponse>,
        tonic::Status,
    > {
        get_service_info::get_service_info(self, request.into_inner())
            .map(Response::new)
            .map_err(Into::into)
    }

    async fn get_objects(
        &self,
        request: tonic::Request<grpc_ledger_service::GetObjectsRequest>,
    ) -> std::result::Result<tonic::Response<Self::GetObjectsStream>, tonic::Status> {
        get_objects::get_objects((*self.reader).clone(), request.into_inner())
            .map(|stream| Response::new(Box::pin(stream) as Self::GetObjectsStream))
            .map_err(Into::into)
    }

    async fn get_transactions(
        &self,
        _request: tonic::Request<grpc_ledger_service::GetTransactionsRequest>,
    ) -> std::result::Result<tonic::Response<Self::GetTransactionsStream>, tonic::Status> {
        // not implemented - return empty stream
        let stream = futures::stream::empty();
        let stream: Self::GetTransactionsStream = Box::pin(stream);
        Ok(Response::new(stream))
    }

    /// Checkpoint operations
    async fn get_checkpoint_data(
        &self,
        _request: tonic::Request<grpc_ledger_service::GetCheckpointDataRequest>,
    ) -> std::result::Result<tonic::Response<Self::StreamCheckpointDataStream>, tonic::Status> {
        let stream = async_stream::try_stream! {
            // not implemented
            yield grpc_ledger_service::CheckpointData {
                payload: None,
            };
        };

        // not implemented
        Ok(Response::new(
            Box::pin(stream) as Self::StreamCheckpointDataStream
        ))
    }

    async fn stream_checkpoint_data(
        &self,
        request: tonic::Request<grpc_ledger_service::CheckpointDataStreamRequest>,
    ) -> std::result::Result<tonic::Response<Self::StreamCheckpointDataStream>, tonic::Status> {
        let req = request.into_inner();
        let start_sequence_number = req.start_sequence_number;
        let end_sequence_number = req.end_sequence_number;

        let stream = self.stream_checkpoint_data(start_sequence_number, end_sequence_number);
        Ok(Response::new(
            Box::pin(stream) as Self::StreamCheckpointDataStream
        ))
    }

    async fn get_epoch(
        &self,
        request: Request<grpc_ledger_service::GetEpochRequest>,
    ) -> Result<Response<grpc_ledger_service::GetEpochResponse>, Status> {
        get_epoch::get_epoch(self, request.into_inner()).map(Response::new)
    }
}
