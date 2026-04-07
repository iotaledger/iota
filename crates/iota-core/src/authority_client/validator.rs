// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::net::SocketAddr;

use async_trait::async_trait;
use iota_types::{
    error::IotaError,
    iota_system_state::IotaSystemState,
    messages_checkpoint::{CheckpointRequest, CheckpointResponse},
    messages_grpc::{
        HandleCapabilityNotificationRequestV1, HandleCapabilityNotificationResponseV1,
        HandleCertificateRequestV1, HandleCertificateResponseV1,
        HandleSoftBundleCertificatesRequestV1, HandleSoftBundleCertificatesResponseV1,
        HandleTransactionResponse, ObjectInfoRequest, ObjectInfoResponse,
        SubmitTransactionsRequest, SubmitTransactionsResponse, SystemStateRequest,
        TransactionInfoRequest, TransactionInfoResponse, ValidatorHealthRequest,
        ValidatorHealthResponse, WaitForEffectsRequest, WaitForEffectsResponse,
    },
    transaction::*,
};
use tonic::IntoRequest;

use crate::authority_client::{NetworkAuthorityClient, insert_metadata};

#[async_trait]
pub trait ValidatorAPI {
    /// Handles a `Transaction`.
    async fn handle_transaction(
        &self,
        transaction: Transaction,
        client_addr: Option<SocketAddr>,
    ) -> Result<HandleTransactionResponse, IotaError>;

    /// Execute a certificate.
    async fn handle_certificate_v1(
        &self,
        request: HandleCertificateRequestV1,
        client_addr: Option<SocketAddr>,
    ) -> Result<HandleCertificateResponseV1, IotaError>;

    /// Execute a Soft Bundle with multiple certificates.
    async fn handle_soft_bundle_certificates_v1(
        &self,
        request: HandleSoftBundleCertificatesRequestV1,
        client_addr: Option<SocketAddr>,
    ) -> Result<HandleSoftBundleCertificatesResponseV1, IotaError>;

    /// Handle Object information requests.
    async fn handle_object_info_request(
        &self,
        request: ObjectInfoRequest,
    ) -> Result<ObjectInfoResponse, IotaError>;

    /// Handles a `TransactionInfoRequest`.
    async fn handle_transaction_info_request(
        &self,
        request: TransactionInfoRequest,
    ) -> Result<TransactionInfoResponse, IotaError>;

    /// Handles a `CheckpointRequest`.
    async fn handle_checkpoint(
        &self,
        request: CheckpointRequest,
    ) -> Result<CheckpointResponse, IotaError>;

    // This API is exclusively used by the benchmark code.
    // Hence it's OK to return a fixed system state type.
    async fn handle_system_state_object(
        &self,
        request: SystemStateRequest,
    ) -> Result<IotaSystemState, IotaError>;

    /// Handle a capability notification from another authority
    async fn handle_capability_notification_v1(
        &self,
        request: HandleCapabilityNotificationRequestV1,
    ) -> Result<HandleCapabilityNotificationResponseV1, IotaError>;

    /// Submit a transaction via the TransactionDriver protocol.
    async fn handle_submit_transactions(
        &self,
        request: SubmitTransactionsRequest,
        client_addr: Option<SocketAddr>,
    ) -> Result<SubmitTransactionsResponse, IotaError>;

    /// Wait for a transaction's effects to be available.
    async fn handle_wait_for_effects(
        &self,
        request: WaitForEffectsRequest,
        client_addr: Option<SocketAddr>,
    ) -> Result<WaitForEffectsResponse, IotaError>;

    /// Query validator health metrics (latency measurement / health check).
    async fn handle_validator_health(
        &self,
        request: ValidatorHealthRequest,
    ) -> Result<ValidatorHealthResponse, IotaError>;
}

#[async_trait]
impl ValidatorAPI for NetworkAuthorityClient {
    /// Handles a `Transaction` .
    async fn handle_transaction(
        &self,
        transaction: Transaction,
        client_addr: Option<SocketAddr>,
    ) -> Result<HandleTransactionResponse, IotaError> {
        let mut request = transaction.into_request();
        insert_metadata(&mut request, client_addr);

        self.client()?
            .transaction(request)
            .await
            .map(tonic::Response::into_inner)
            .map_err(Into::into)
    }

    async fn handle_certificate_v1(
        &self,
        request: HandleCertificateRequestV1,
        client_addr: Option<SocketAddr>,
    ) -> Result<HandleCertificateResponseV1, IotaError> {
        let mut request = request.into_request();
        insert_metadata(&mut request, client_addr);

        let response = self
            .client()?
            .handle_certificate_v1(request)
            .await
            .map(tonic::Response::into_inner);

        response.map_err(Into::into)
    }

    async fn handle_soft_bundle_certificates_v1(
        &self,
        request: HandleSoftBundleCertificatesRequestV1,
        client_addr: Option<SocketAddr>,
    ) -> Result<HandleSoftBundleCertificatesResponseV1, IotaError> {
        let mut request = request.into_request();
        insert_metadata(&mut request, client_addr);

        let response = self
            .client()?
            .handle_soft_bundle_certificates_v1(request)
            .await
            .map(tonic::Response::into_inner);

        response.map_err(Into::into)
    }

    /// Handles a `ObjectInfoRequest` .
    async fn handle_object_info_request(
        &self,
        request: ObjectInfoRequest,
    ) -> Result<ObjectInfoResponse, IotaError> {
        self.client()?
            .object_info(request)
            .await
            .map(tonic::Response::into_inner)
            .map_err(Into::into)
    }

    /// Handles a `TransactionInfoRequest` .
    async fn handle_transaction_info_request(
        &self,
        request: TransactionInfoRequest,
    ) -> Result<TransactionInfoResponse, IotaError> {
        self.client()?
            .transaction_info(request)
            .await
            .map(tonic::Response::into_inner)
            .map_err(Into::into)
    }

    /// Handles a `CheckpointRequest` .
    async fn handle_checkpoint(
        &self,
        request: CheckpointRequest,
    ) -> Result<CheckpointResponse, IotaError> {
        self.client()?
            .checkpoint(request)
            .await
            .map(tonic::Response::into_inner)
            .map_err(Into::into)
    }

    /// This API is exclusively used by the benchmark code.
    async fn handle_system_state_object(
        &self,
        request: SystemStateRequest,
    ) -> Result<IotaSystemState, IotaError> {
        self.client()?
            .get_system_state_object(request)
            .await
            .map(tonic::Response::into_inner)
            .map_err(Into::into)
    }

    async fn handle_capability_notification_v1(
        &self,
        request: HandleCapabilityNotificationRequestV1,
    ) -> Result<HandleCapabilityNotificationResponseV1, IotaError> {
        self.client()?
            .handle_capability_notification_v1(request)
            .await
            .map(tonic::Response::into_inner)
            .map_err(Into::into)
    }

    async fn handle_submit_transactions(
        &self,
        request: SubmitTransactionsRequest,
        client_addr: Option<SocketAddr>,
    ) -> Result<SubmitTransactionsResponse, IotaError> {
        use iota_types::messages_grpc::RawSubmitTransactionsRequest;
        let raw_request: RawSubmitTransactionsRequest = request.try_into()?;
        let mut grpc_request = raw_request.into_request();
        insert_metadata(&mut grpc_request, client_addr);

        let raw_response = self
            .client()?
            .handle_submit_transactions(grpc_request)
            .await
            .map(tonic::Response::into_inner)
            .map_err(IotaError::from)?;
        raw_response.try_into()
    }

    async fn handle_wait_for_effects(
        &self,
        request: WaitForEffectsRequest,
        _client_addr: Option<SocketAddr>,
    ) -> Result<WaitForEffectsResponse, IotaError> {
        use iota_types::messages_grpc::RawWaitForEffectsRequest;
        let raw_request: RawWaitForEffectsRequest = request.try_into()?;

        let raw_response = self
            .client()?
            .handle_wait_for_effects(raw_request.into_request())
            .await
            .map(tonic::Response::into_inner)
            .map_err(IotaError::from)?;
        raw_response.try_into()
    }

    async fn handle_validator_health(
        &self,
        request: ValidatorHealthRequest,
    ) -> Result<ValidatorHealthResponse, IotaError> {
        use iota_types::messages_grpc::RawValidatorHealthRequest;
        let raw_request: RawValidatorHealthRequest = request.try_into()?;

        let raw_response = self
            .client()?
            .handle_validator_health(raw_request.into_request())
            .await
            .map(tonic::Response::into_inner)
            .map_err(IotaError::from)?;
        raw_response.try_into()
    }
}
