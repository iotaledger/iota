// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::net::SocketAddr;

use async_trait::async_trait;
use futures::StreamExt;
use iota_types::{
    digests::TransactionDigest,
    error::IotaError,
    messages_grpc::{
        GetTxStatusRequest, HandleCapabilityNotificationRequestV1,
        HandleCapabilityNotificationResponseV1, SubmitTransactionsRequest, TxStatusUpdate,
        ValidatorHealthRequest, ValidatorHealthResponse,
    },
};
use tonic::IntoRequest;

use crate::authority_client::{NetworkAuthorityClient, insert_metadata};

#[async_trait]
pub trait ValidatorV2API {
    /// Submit transactions and collect all streamed status updates.
    async fn submit_tx(
        &self,
        request: SubmitTransactionsRequest,
        client_addr: Option<SocketAddr>,
    ) -> Result<Vec<(TransactionDigest, TxStatusUpdate)>, IotaError>;

    /// Query transaction status and collect all streamed status updates.
    async fn get_tx_status(
        &self,
        request: GetTxStatusRequest,
        client_addr: Option<SocketAddr>,
    ) -> Result<Vec<(TransactionDigest, TxStatusUpdate)>, IotaError>;

    /// Notify capabilities via the V2 endpoint.
    async fn notify_capabilities_v2(
        &self,
        request: HandleCapabilityNotificationRequestV1,
    ) -> Result<HandleCapabilityNotificationResponseV1, IotaError>;

    /// Health check endpoint.
    async fn health_check(
        &self,
        request: ValidatorHealthRequest,
    ) -> Result<ValidatorHealthResponse, IotaError>;
}

#[async_trait]
impl ValidatorV2API for NetworkAuthorityClient {
    async fn submit_tx(
        &self,
        request: SubmitTransactionsRequest,
        client_addr: Option<SocketAddr>,
    ) -> Result<Vec<(TransactionDigest, TxStatusUpdate)>, IotaError> {
        let proto: iota_network::api::SubmitTxRequest = request.try_into()?;
        let mut grpc_request = proto.into_request();
        insert_metadata(&mut grpc_request, client_addr);

        let response = self
            .v2_client()?
            .submit_tx(grpc_request)
            .await
            .map_err(IotaError::from)?;

        collect_tx_status_stream(response.into_inner()).await
    }

    async fn get_tx_status(
        &self,
        request: GetTxStatusRequest,
        client_addr: Option<SocketAddr>,
    ) -> Result<Vec<(TransactionDigest, TxStatusUpdate)>, IotaError> {
        let proto: iota_network::api::GetTxStatusRequest = request.try_into()?;
        let mut grpc_request = proto.into_request();
        insert_metadata(&mut grpc_request, client_addr);

        let response = self
            .v2_client()?
            .get_tx_status(grpc_request)
            .await
            .map_err(IotaError::from)?;

        collect_tx_status_stream(response.into_inner()).await
    }

    async fn notify_capabilities_v2(
        &self,
        request: HandleCapabilityNotificationRequestV1,
    ) -> Result<HandleCapabilityNotificationResponseV1, IotaError> {
        let proto_request: iota_network::api::NotifyCapabilitiesRequest = request.try_into()?;
        let response = self
            .v2_client()?
            .notify_capabilities(proto_request)
            .await
            .map_err(IotaError::from)?;

        Ok(response.into_inner().into())
    }

    async fn health_check(
        &self,
        request: ValidatorHealthRequest,
    ) -> Result<ValidatorHealthResponse, IotaError> {
        let proto_request: iota_network::api::HealthCheckRequest = request.into();
        let response = self
            .v2_client()?
            .health_check(proto_request)
            .await
            .map_err(IotaError::from)?;

        Ok(response.into_inner().into())
    }
}

/// Collects all items from a `TxStatus` stream into a `Vec`.
// TODO(#11180): return per-item results so a mid-stream transport or
// deserialization error does not discard already-received statuses.
async fn collect_tx_status_stream(
    mut stream: tonic::Streaming<iota_network::api::TxStatus>,
) -> Result<Vec<(TransactionDigest, TxStatusUpdate)>, IotaError> {
    let mut results = Vec::new();
    while let Some(item) = stream.next().await {
        let tx_status = item.map_err(IotaError::from)?;
        results.push(tx_status.try_into()?);
    }
    Ok(results)
}
