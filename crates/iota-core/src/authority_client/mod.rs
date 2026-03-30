// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, net::SocketAddr, time::Duration};

use anyhow::anyhow;
use iota_network::{
    api::ValidatorClient,
    tonic,
    tonic::{metadata::KeyAndValueRef, transport::Channel},
};
use iota_network_stack::config::Config;
use iota_types::{
    base_types::AuthorityName,
    committee::CommitteeWithNetworkMetadata,
    crypto::NetworkPublicKey,
    error::{IotaError, IotaResult},
    multiaddr::Multiaddr,
};
use tap::TapFallible;

use crate::authority_client::{
    validator::ValidatorAPI, validator_peer::ValidatorPeerAPI, validator_v2::ValidatorV2API,
};

pub mod validator;
pub mod validator_peer;
pub mod validator_v2;

/// A client for the network authority.
#[derive(Clone)]
pub struct NetworkAuthorityClient {
    client: IotaResult<ValidatorClient<Channel>>,
}

impl NetworkAuthorityClient {
    pub async fn connect(
        address: &Multiaddr,
        tls_target: Option<NetworkPublicKey>,
    ) -> anyhow::Result<Self> {
        let tls_config = tls_target.map(|tls_target| {
            iota_tls::create_rustls_client_config(
                tls_target,
                iota_tls::IOTA_VALIDATOR_SERVER_NAME.to_string(),
                None,
            )
        });
        let channel = iota_network_stack::client::connect(address, tls_config)
            .await
            .map_err(|err| anyhow!(err.to_string()))?;
        Ok(Self::new(channel))
    }

    pub fn connect_lazy(address: &Multiaddr, tls_target: Option<NetworkPublicKey>) -> Self {
        let tls_config = tls_target.map(|tls_target| {
            iota_tls::create_rustls_client_config(
                tls_target,
                iota_tls::IOTA_VALIDATOR_SERVER_NAME.to_string(),
                None,
            )
        });
        let client: IotaResult<_> = iota_network_stack::client::connect_lazy(address, tls_config)
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

pub trait AuthorityAPI: ValidatorAPI + ValidatorPeerAPI + ValidatorV2API {}

impl<T> AuthorityAPI for T where T: ValidatorAPI + ValidatorPeerAPI + ValidatorV2API {}

/// Creates authority clients with network configuration.
pub fn make_network_authority_clients_with_network_config(
    committee: &CommitteeWithNetworkMetadata,
    network_config: &Config,
) -> BTreeMap<AuthorityName, NetworkAuthorityClient> {
    let mut authority_clients = BTreeMap::new();
    for (name, (_state, network_metadata)) in committee.validators() {
        let address = network_metadata
            .network_address
            .clone()
            .rewrite_udp_to_tcp()
            .rewrite_http_to_https();
        let tls_config = network_metadata
            .network_public_key
            .as_ref()
            .map(|key| {
                iota_tls::create_rustls_client_config(
                    key.clone(),
                    iota_tls::IOTA_VALIDATOR_SERVER_NAME.to_string(),
                    None,
                )
            })
            .ok_or(IotaError::from("network public key is not available"));
        let maybe_channel = tls_config
            .and_then(|tls_config| {
                network_config
                    .connect_lazy(&address, Some(tls_config))
                    .map_err(|e| e.to_string().into())
            })
            .tap_err(|e| {
                tracing::error!(
                    address = %address,
                    name = %name,
                    "unable to create authority client: {e}"
                )
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
