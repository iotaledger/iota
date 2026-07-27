// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod get_checkpoint;
mod get_epoch;
mod get_health;
mod get_objects;
mod get_service_info;
mod get_transactions;

use std::sync::Arc;

use futures::{Stream, StreamExt};
use iota_config::node::GrpcApiConfig;
use iota_grpc_types::v1::ledger_service::{self as grpc_ledger_service};
use iota_protocol_config::Chain;
use iota_types::digests::ChainIdentifier;
use tokio_util::sync::CancellationToken;
use tonic::{Code, Request, Response, Status};

use crate::{traffic_control::TallyHandle, types::*};

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

/// Reject a read batch whose item count exceeds `max`. An empty batch is
/// allowed and yields an empty stream. Bounding the count also bounds the
/// per-item traffic-control tally so a large batch cannot flood the tally
/// channel.
fn validate_read_batch_size(item_count: usize, max: u32) -> Result<(), Status> {
    if item_count > max as usize {
        return Err(Status::invalid_argument(format!(
            "batch size {item_count} exceeds maximum allowed ({max})"
        )));
    }
    Ok(())
}

/// Charge a streaming batch read to the traffic controller as its response
/// stream is consumed.
///
/// The batch's items are streamed lazily and invisible to the transport-level
/// traffic control. The request is marked as accounted so the layer skips its
/// default per-request tally, and is instead charged one request per produced
/// item, so batching cannot dilute the spam rate; an empty batch produces no
/// per-item tallies and is charged as one request up front instead. Per-item
/// read failures ride inside `Ok` responses and are tallied as `Ok`: a client
/// cannot know an object or transaction was pruned, so they must not feed the
/// error policy. A stream-level error terminates the stream and is tallied by
/// its status code, like a unary handler error.
fn tally_read_stream<T>(
    stream: impl Stream<Item = Result<T, Status>> + Send,
    tally_handle: Option<TallyHandle>,
    request_item_count: usize,
    response_item_count: impl Fn(&T) -> usize + Send,
) -> impl Stream<Item = Result<T, Status>> + Send {
    if let Some(tally_handle) = &tally_handle {
        if request_item_count == 0 {
            tally_handle.tally_item(Code::Ok);
        } else {
            tally_handle.mark_accounted();
        }
    }
    stream.map(move |result| {
        if let Some(tally_handle) = &tally_handle {
            match &result {
                Ok(response) => tally_handle
                    .tally_items(std::iter::repeat_n(Code::Ok, response_item_count(response))),
                Err(status) => tally_handle.tally_item(status.code()),
            }
        }
        result
    })
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
        let tally_handle = request.extensions().get::<TallyHandle>().cloned();
        let request = request.into_inner();
        let item_count = request
            .requests
            .as_ref()
            .map_or(0, |batch| batch.requests.len());
        validate_read_batch_size(item_count, self.config.max_get_objects_batch_size)?;
        let stream =
            get_objects::get_objects(self.reader.clone(), request).map_err(tonic::Status::from)?;
        let stream = tally_read_stream(stream, tally_handle, item_count, |response| {
            response.objects.len()
        });
        let response = Response::new(Box::pin(stream) as Self::GetObjectsStream);
        Ok(append_info_headers!(response, self.reader.clone()))
    }

    async fn get_transactions(
        &self,
        request: tonic::Request<grpc_ledger_service::GetTransactionsRequest>,
    ) -> std::result::Result<tonic::Response<Self::GetTransactionsStream>, tonic::Status> {
        let tally_handle = request.extensions().get::<TallyHandle>().cloned();
        let request = request.into_inner();
        let item_count = request
            .requests
            .as_ref()
            .map_or(0, |batch| batch.requests.len());
        validate_read_batch_size(item_count, self.config.max_get_transactions_batch_size)?;
        let stream =
            get_transactions::get_transactions(self.reader.clone(), self.config.clone(), request)
                .map_err(tonic::Status::from)?;
        let stream = tally_read_stream(stream, tally_handle, item_count, |response| {
            response.transaction_results.len()
        });
        let response = Response::new(Box::pin(stream) as Self::GetTransactionsStream);
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
