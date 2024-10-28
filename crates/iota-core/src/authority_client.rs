// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, net::SocketAddr, time::Duration};

use anyhow::anyhow;
use async_trait::async_trait;
use iota_network::{
    api::ValidatorClient,
    tonic,
    tonic::{metadata::KeyAndValueRef, transport::Channel},
};
use iota_network_stack::config::Config;
use iota_types::{
    base_types::AuthorityName,
    committee::CommitteeWithNetworkMetadata,
    error::{IotaError, IotaResult},
    iota_system_state::IotaSystemState,
    messages_checkpoint::{CheckpointRequest, CheckpointResponse},
    messages_grpc::{
        HandleCertificateRequestV3, HandleCertificateResponseV2, HandleCertificateResponseV3,
        HandleSoftBundleCertificatesRequestV3, HandleSoftBundleCertificatesResponseV3,
        HandleTransactionResponse, ObjectInfoRequest, ObjectInfoResponse, SystemStateRequest,
        TransactionInfoRequest, TransactionInfoResponse,
    },
    multiaddr::Multiaddr,
    transaction::*,
};

use crate::authority_client::tonic::IntoRequest;

#[async_trait]
pub trait AuthorityAPI {
    /// Handles a `Transaction` for this account.
    async fn handle_transaction(
        &self,
        transaction: Transaction,
        client_addr: Option<SocketAddr>,
    ) -> Result<HandleTransactionResponse, IotaError>;

    /// Handles a `CertifiedTransaction` for this account.
    async fn handle_certificate_v2(
        &self,
        certificate: CertifiedTransaction,
        client_addr: Option<SocketAddr>,
    ) -> Result<HandleCertificateResponseV2, IotaError>;

    /// Execute a certificate.
    async fn handle_certificate_v3(
        &self,
        request: HandleCertificateRequestV3,
        client_addr: Option<SocketAddr>,
    ) -> Result<HandleCertificateResponseV3, IotaError>;

    /// Execute a Soft Bundle with multiple certificates.
    async fn handle_soft_bundle_certificates_v3(
        &self,
        request: HandleSoftBundleCertificatesRequestV3,
        client_addr: Option<SocketAddr>,
    ) -> Result<HandleSoftBundleCertificatesResponseV3, IotaError>;

    /// Handle Object information requests for this account.
    async fn handle_object_info_request(
        &self,
        request: ObjectInfoRequest,
    ) -> Result<ObjectInfoResponse, IotaError>;

    /// Handles a `TransactionInfoRequest` for this account.
    async fn handle_transaction_info_request(
        &self,
        request: TransactionInfoRequest,
    ) -> Result<TransactionInfoResponse, IotaError>;

    /// Handles a `CheckpointRequest` for this account.
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
}

/// A client for the network authority.
#[derive(Clone)]
pub struct NetworkAuthorityClient {
    client: IotaResult<ValidatorClient<Channel>>,
}

impl NetworkAuthorityClient {
    /// Connects to a client address.
    pub async fn connect(address: &Multiaddr) -> anyhow::Result<Self> {
        let channel = iota_network_stack::client::connect(address)
            .await
            .map_err(|err| anyhow!(err.to_string()))?;
        Ok(Self::new(channel))
    }

    /// Connects to a client address lazily.
    pub fn connect_lazy(address: &Multiaddr) -> Self {
        let client: IotaResult<_> = iota_network_stack::client::connect_lazy(address)
            .map(ValidatorClient::new)
            .map_err(|err| err.to_string().into());
        Self { client }
    }

    /// Creates a new client with a `transport` channel.
    pub fn new(channel: Channel) -> Self {
        Self {
            client: Ok(ValidatorClient::new(channel)),
        }
    }

    /// Creates a new client with a lazy `transport` channel.
    fn new_lazy(client: IotaResult<Channel>) -> Self {
        Self {
            client: client.map(ValidatorClient::new),
        }
    }

    fn client(&self) -> IotaResult<ValidatorClient<Channel>> {
        self.client.clone()
    }
}

#[async_trait]
impl AuthorityAPI for NetworkAuthorityClient {
    /// Handles a `Transaction` for this account.
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

    /// Handles a `CertifiedTransaction` for this account.
    async fn handle_certificate_v2(
        &self,
        certificate: CertifiedTransaction,
        client_addr: Option<SocketAddr>,
    ) -> Result<HandleCertificateResponseV2, IotaError> {
        let mut request = certificate.into_request();
        insert_metadata(&mut request, client_addr);

        let response = self
            .client()?
            .handle_certificate_v2(request)
            .await
            .map(tonic::Response::into_inner);

        response.map_err(Into::into)
    }

    async fn handle_certificate_v3(
        &self,
        request: HandleCertificateRequestV3,
        client_addr: Option<SocketAddr>,
    ) -> Result<HandleCertificateResponseV3, IotaError> {
        let mut request = request.into_request();
        insert_metadata(&mut request, client_addr);

        let response = self
            .client()?
            .handle_certificate_v3(request)
            .await
            .map(tonic::Response::into_inner);

        response.map_err(Into::into)
    }

    async fn handle_soft_bundle_certificates_v3(
        &self,
        request: HandleSoftBundleCertificatesRequestV3,
        client_addr: Option<SocketAddr>,
    ) -> Result<HandleSoftBundleCertificatesResponseV3, IotaError> {
        let mut request = request.into_request();
        insert_metadata(&mut request, client_addr);

        let response = self
            .client()?
            .handle_soft_bundle_certificates_v3(request)
            .await
            .map(tonic::Response::into_inner);

        response.map_err(Into::into)
    }

    /// Handles a `ObjectInfoRequest` for this account.
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

    /// Handles a `TransactionInfoRequest` for this account.
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

    /// Handles a `CheckpointRequest` for this account.
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
}

/// Creates authority clients with network configuration.
pub fn make_network_authority_clients_with_network_config(
    committee: &CommitteeWithNetworkMetadata,
    network_config: &Config,
) -> BTreeMap<AuthorityName, NetworkAuthorityClient> {
    let mut authority_clients = BTreeMap::new();
    for (name, (_state, network_metadata)) in committee.validators() {
        let address = network_metadata.network_address.clone();
        let address = address.rewrite_udp_to_tcp();
        let maybe_channel = network_config.connect_lazy(&address).map_err(|e| {
            tracing::error!(
                address = %address,
                name = %name,
                "unable to create authority client: {e}"
            );
            e.to_string().into()
        });
        let client = NetworkAuthorityClient::new_lazy(maybe_channel);
        authority_clients.insert(*name, client);
    }
    authority_clients
}

/// Creates authority clients with a timeout configuration.
pub fn make_authority_clients_with_timeout_config(
    committee: &CommitteeWithNetworkMetadata,
    connect_timeout: Duration,
    request_timeout: Duration,
) -> BTreeMap<AuthorityName, NetworkAuthorityClient> {
    let mut network_config = iota_network_stack::config::Config::new();
    network_config.connect_timeout = Some(connect_timeout);
    network_config.request_timeout = Some(request_timeout);
    make_network_authority_clients_with_network_config(committee, &network_config)
}

fn insert_metadata<T>(request: &mut tonic::Request<T>, client_addr: Option<SocketAddr>) {
    if let Some(client_addr) = client_addr {
        let mut metadata = tonic::metadata::MetadataMap::new();
        metadata.insert("x-forwarded-for", client_addr.to_string().parse().unwrap());
        metadata
            .iter()
            .for_each(|key_and_value| match key_and_value {
                KeyAndValueRef::Ascii(key, value) => {
                    request.metadata_mut().insert(key, value.clone());
                }
                KeyAndValueRef::Binary(key, value) => {
                    request.metadata_mut().insert_bin(key, value.clone());
                }
            });
    }
}
