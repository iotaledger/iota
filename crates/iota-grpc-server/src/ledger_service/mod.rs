// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod get_checkpoint;
mod get_epoch;
mod get_health;
mod get_objects;
mod get_service_info;
mod get_transactions;

use std::sync::Arc;

use iota_config::node::GrpcApiConfig;
use iota_grpc_types::v1::ledger_service::{self as grpc_ledger_service};
use iota_protocol_config::Chain;
use iota_types::digests::ChainIdentifier;
use tokio_util::sync::CancellationToken;
use tonic::{Request, Response, Status};

use crate::types::*;

pub struct LedgerGrpcService {
    pub config: GrpcApiConfig,
    pub reader: Arc<GrpcReader>,
    pub checkpoint_data_broadcaster: GrpcCheckpointDataBroadcaster,
    pub cancellation_token: CancellationToken,
    pub chain_id: ChainIdentifier,
    pub chain: Chain,
}

impl LedgerGrpcService {
    pub fn new(
        config: GrpcApiConfig,
        reader: Arc<GrpcReader>,
        checkpoint_data_broadcaster: GrpcCheckpointDataBroadcaster,
        cancellation_token: CancellationToken,
        chain_id: ChainIdentifier,
    ) -> Self {
        Self {
            config,
            reader,
            checkpoint_data_broadcaster,
            cancellation_token,
            chain_id,
            chain: chain_id.chain(),
        }
    }
}

#[tonic::async_trait]
impl grpc_ledger_service::ledger_service_server::LedgerService for LedgerGrpcService {
    type GetObjectsStream = crate::types::GetObjectsStream;
    type GetTransactionsStream = crate::types::GetTransactionsStream;
    type GetCheckpointStream = crate::types::GetCheckpointStream;
    type StreamCheckpointsStream = crate::types::StreamCheckpointsStream;

    async fn get_health(
        &self,
        request: tonic::Request<grpc_ledger_service::GetHealthRequest>,
    ) -> std::result::Result<tonic::Response<grpc_ledger_service::GetHealthResponse>, tonic::Status>
    {
        let response = get_health::get_health(self, request.into_inner())
            .map(Response::new)
            .map_err(tonic::Status::from)?;
        Ok(append_info_headers!(response, self.reader.clone()))
    }

    /// Query the service for general information about its current state.
    async fn get_service_info(
        &self,
        request: tonic::Request<grpc_ledger_service::GetServiceInfoRequest>,
    ) -> std::result::Result<
        tonic::Response<grpc_ledger_service::GetServiceInfoResponse>,
        tonic::Status,
    > {
        let response = get_service_info::get_service_info(self, request.into_inner())
            .map(Response::new)
            .map_err(tonic::Status::from)?;
        Ok(append_info_headers!(response, self.reader.clone()))
    }

    async fn get_objects(
        &self,
        request: tonic::Request<grpc_ledger_service::GetObjectsRequest>,
    ) -> std::result::Result<tonic::Response<Self::GetObjectsStream>, tonic::Status> {
        let response = get_objects::get_objects(self.reader.clone(), request.into_inner())
            .map(|stream| Response::new(Box::pin(stream) as Self::GetObjectsStream))
            .map_err(tonic::Status::from)?;
        Ok(append_info_headers!(response, self.reader.clone()))
    }

    async fn get_transactions(
        &self,
        request: tonic::Request<grpc_ledger_service::GetTransactionsRequest>,
    ) -> std::result::Result<tonic::Response<Self::GetTransactionsStream>, tonic::Status> {
        let response = get_transactions::get_transactions(
            self.reader.clone(),
            self.config.clone(),
            request.into_inner(),
        )
        .map(|stream| Response::new(Box::pin(stream) as Self::GetTransactionsStream))
        .map_err(tonic::Status::from)?;
        Ok(append_info_headers!(response, self.reader.clone()))
    }

    /// Checkpoint operations
    async fn get_checkpoint(
        &self,
        request: tonic::Request<grpc_ledger_service::GetCheckpointRequest>,
    ) -> std::result::Result<tonic::Response<Self::GetCheckpointStream>, tonic::Status> {
        let response = get_checkpoint::get_checkpoint(self, request)
            .map(|stream| Response::new(Box::pin(stream) as Self::GetCheckpointStream))
            .map_err(tonic::Status::from)?;
        Ok(append_info_headers!(response, self.reader.clone()))
    }

    async fn stream_checkpoints(
        &self,
        request: tonic::Request<grpc_ledger_service::StreamCheckpointsRequest>,
    ) -> std::result::Result<tonic::Response<Self::StreamCheckpointsStream>, tonic::Status> {
        let response = get_checkpoint::stream_checkpoints(self, request)
            .map(|stream| Response::new(Box::pin(stream) as Self::StreamCheckpointsStream))
            .map_err(tonic::Status::from)?;
        Ok(append_info_headers!(response, self.reader.clone()))
    }

    async fn get_epoch(
        &self,
        request: Request<grpc_ledger_service::GetEpochRequest>,
    ) -> Result<Response<grpc_ledger_service::GetEpochResponse>, Status> {
        let response = get_epoch::get_epoch(self, request.into_inner()).map(Response::new)?;
        Ok(append_info_headers!(response, self.reader.clone()))
    }
}
