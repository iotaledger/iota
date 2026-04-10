// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;
use iota_types::{
    error::IotaError,
    messages_checkpoint::{CheckpointRequest, CheckpointResponse},
};

use crate::authority_client::NetworkAuthorityClient;

#[async_trait]
pub trait ValidatorPeerAPI {
    /// Fetch a checkpoint via the ValidatorPeer endpoint.
    async fn get_checkpoint_v2(
        &self,
        request: CheckpointRequest,
    ) -> Result<CheckpointResponse, IotaError>;
}

#[async_trait]
impl ValidatorPeerAPI for NetworkAuthorityClient {
    async fn get_checkpoint_v2(
        &self,
        request: CheckpointRequest,
    ) -> Result<CheckpointResponse, IotaError> {
        let proto_request: iota_network::api::GetCheckpointRequest = request.into();
        let response = self
            .peer_client()?
            .get_checkpoint(proto_request)
            .await
            .map_err(IotaError::from)?;

        response.into_inner().try_into()
    }
}
