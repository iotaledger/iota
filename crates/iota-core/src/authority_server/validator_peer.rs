// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_network::{
    api::{GetCheckpointRequest, GetCheckpointResponse, ValidatorPeer},
    tonic,
};
use iota_types::{
    messages_checkpoint::{CheckpointRequest, CheckpointResponse},
    traffic_control::Weight,
};

use crate::authority_server::ValidatorService;

impl ValidatorService {
    async fn get_checkpoint_impl(
        &self,
        request: CheckpointRequest,
    ) -> Result<(CheckpointResponse, Weight), tonic::Status> {
        let response = self.state.handle_checkpoint_request(&request)?;
        Ok((response, Weight::one()))
    }
}

#[async_trait::async_trait]
impl ValidatorPeer for ValidatorService {
    async fn get_checkpoint(
        &self,
        request: tonic::Request<GetCheckpointRequest>,
    ) -> Result<tonic::Response<GetCheckpointResponse>, tonic::Status> {
        let (req, ip) = self.pre_handle(request).await?;
        self.post_handle_unary(ip, self.get_checkpoint_impl(req).await)
    }
}
