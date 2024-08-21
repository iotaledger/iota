// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, time::Duration};

use anyhow::anyhow;
use async_trait::async_trait;
use iota_network::{api::ValidatorClient, tonic, tonic::transport::Channel};
use iota_types::{
    base_types::AuthorityName,
    committee::CommitteeWithNetworkMetadata,
    error::IotaError,
    iota_system_state::IotaSystemState,
    messages_checkpoint::{
        CheckpointRequest, CheckpointRequestV2, CheckpointResponse, CheckpointResponseV2,
    },
    messages_grpc::{
        HandleCertificateResponseV2, HandleTransactionResponse, ObjectInfoRequest,
        ObjectInfoResponse, SystemStateRequest, TransactionInfoRequest, TransactionInfoResponse,
    },
    multiaddr::Multiaddr,
    transaction::*,
};
use iota_network_stack::config::Config;

#[async_trait]
pub trait AuthorityAPI {
    /// Initiate a new transaction to a Iota or Primary account.
    async fn handle_transaction(
        &self,
        transaction: Transaction,
    ) -> Result<HandleTransactionResponse, IotaError>;

    /// Execute a certificate.
    async fn handle_certificate_v2(
        &self,
        certificate: CertifiedTransaction,
    ) -> Result<HandleCertificateResponseV2, IotaError>;

    /// Handle Object information requests for this account.
    async fn handle_object_info_request(
        &self,
        request: ObjectInfoRequest,
    ) -> Result<ObjectInfoResponse, IotaError>;

    /// Handle Object information requests for this account.
    async fn handle_transaction_info_request(
        &self,
        request: TransactionInfoRequest,
    ) -> Result<TransactionInfoResponse, IotaError>;

    async fn handle_checkpoint(
        &self,
        request: CheckpointRequest,
    ) -> Result<CheckpointResponse, IotaError>;

    async fn handle_checkpoint_v2(
        &self,
        request: CheckpointRequestV2,
    ) -> Result<CheckpointResponseV2, IotaError>;

    // This API is exclusively used by the benchmark code.
    // Hence it's OK to return a fixed system state type.
    async fn handle_system_state_object(
        &self,
        request: SystemStateRequest,
    ) -> Result<IotaSystemState, IotaError>;
}

#[derive(Clone)]
pub struct NetworkAuthorityClient {
    client: ValidatorClient<Channel>,
}

impl NetworkAuthorityClient {
    pub async fn connect(address: &Multiaddr) -> anyhow::Result<Self> {
        let channel = iota_network_stack::client::connect(address)
            .await
            .map_err(|err| anyhow!(err.to_string()))?;
        Ok(Self::new(channel))
    }

    pub fn connect_lazy(address: &Multiaddr) -> anyhow::Result<Self> {
        let channel = iota_network_stack::client::connect_lazy(address)
            .map_err(|err| anyhow!(err.to_string()))?;
        Ok(Self::new(channel))
    }

    pub fn new(channel: Channel) -> Self {
        Self {
            client: ValidatorClient::new(channel),
        }
    }

    fn client(&self) -> ValidatorClient<Channel> {
        self.client.clone()
    }
}

#[async_trait]
impl AuthorityAPI for NetworkAuthorityClient {
    /// Initiate a new transfer to a Iota or Primary account.
    async fn handle_transaction(
        &self,
        transaction: Transaction,
    ) -> Result<HandleTransactionResponse, IotaError> {
        self.client()
            .transaction(transaction)
            .await
            .map(tonic::Response::into_inner)
            .map_err(Into::into)
    }

    /// Execute a certificate.
    async fn handle_certificate_v2(
        &self,
        certificate: CertifiedTransaction,
    ) -> Result<HandleCertificateResponseV2, IotaError> {
        let response = self
            .client()
            .handle_certificate_v2(certificate.clone())
            .await
            .map(tonic::Response::into_inner);

        response.map_err(Into::into)
    }

    async fn handle_object_info_request(
        &self,
        request: ObjectInfoRequest,
    ) -> Result<ObjectInfoResponse, IotaError> {
        self.client()
            .object_info(request)
            .await
            .map(tonic::Response::into_inner)
            .map_err(Into::into)
    }

    /// Handle Object information requests for this account.
    async fn handle_transaction_info_request(
        &self,
        request: TransactionInfoRequest,
    ) -> Result<TransactionInfoResponse, IotaError> {
        self.client()
            .transaction_info(request)
            .await
            .map(tonic::Response::into_inner)
            .map_err(Into::into)
    }

    /// Handle Object information requests for this account.
    async fn handle_checkpoint(
        &self,
        request: CheckpointRequest,
    ) -> Result<CheckpointResponse, IotaError> {
        self.client()
            .checkpoint(request)
            .await
            .map(tonic::Response::into_inner)
            .map_err(Into::into)
    }

    /// Handle Object information requests for this account.
    async fn handle_checkpoint_v2(
        &self,
        request: CheckpointRequestV2,
    ) -> Result<CheckpointResponseV2, IotaError> {
        self.client()
            .checkpoint_v2(request)
            .await
            .map(tonic::Response::into_inner)
            .map_err(Into::into)
    }

    async fn handle_system_state_object(
        &self,
        request: SystemStateRequest,
    ) -> Result<IotaSystemState, IotaError> {
        self.client()
            .get_system_state_object(request)
            .await
            .map(tonic::Response::into_inner)
            .map_err(Into::into)
    }
}

pub fn make_network_authority_clients_with_network_config(
    committee: &CommitteeWithNetworkMetadata,
    network_config: &Config,
) -> anyhow::Result<BTreeMap<AuthorityName, NetworkAuthorityClient>> {
    let mut authority_clients = BTreeMap::new();
    for (name, _stakes) in &committee.committee.voting_rights {
        let address = &committee
            .network_metadata
            .get(name)
            .ok_or_else(|| {
                IotaError::from("Missing network metadata in CommitteeWithNetworkMetadata")
            })?
            .network_address;
        let channel = network_config
            .connect_lazy(address)
            .map_err(|err| anyhow!(err.to_string()))?;
        let client = NetworkAuthorityClient::new(channel);
        authority_clients.insert(*name, client);
    }
    Ok(authority_clients)
}

pub fn make_authority_clients_with_timeout_config(
    committee: &CommitteeWithNetworkMetadata,
    connect_timeout: Duration,
    request_timeout: Duration,
) -> anyhow::Result<BTreeMap<AuthorityName, NetworkAuthorityClient>> {
    let mut network_config = iota_network_stack::config::Config::new();
    network_config.connect_timeout = Some(connect_timeout);
    network_config.request_timeout = Some(request_timeout);
    make_network_authority_clients_with_network_config(committee, &network_config)
}
