// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_network::{
    api::{GetCheckpointRequest, GetCheckpointResponse, ValidatorPeer},
    tonic,
};

use crate::authority_server::ValidatorService;

#[async_trait::async_trait]
impl ValidatorPeer for ValidatorService {
    async fn get_checkpoint(
        &self,
        _request: tonic::Request<GetCheckpointRequest>,
    ) -> Result<tonic::Response<GetCheckpointResponse>, tonic::Status> {
        todo!()
    }
}
