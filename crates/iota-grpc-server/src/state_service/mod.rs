// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod get_coin_info;
mod list_dynamic_fields;
mod list_owned_objects;

use std::sync::Arc;

use iota_grpc_types::v1::state_service::{self as grpc_state_service};
use tonic::Response;

use crate::types::*;

pub struct StateGrpcService {
    pub reader: Arc<GrpcReader>,
}

impl StateGrpcService {
    pub fn new(reader: Arc<GrpcReader>) -> Self {
        Self { reader }
    }
}

#[tonic::async_trait]
impl grpc_state_service::state_service_server::StateService for StateGrpcService {
    async fn list_dynamic_fields(
        &self,
        request: tonic::Request<grpc_state_service::ListDynamicFieldsRequest>,
    ) -> std::result::Result<
        tonic::Response<grpc_state_service::ListDynamicFieldsResponse>,
        tonic::Status,
    > {
        let response =
            list_dynamic_fields::list_dynamic_fields(self.reader.clone(), request.into_inner())
                .map(Response::new)
                .map_err(tonic::Status::from)?;
        Ok(append_info_headers!(response, self.reader.clone()))
    }

    async fn list_owned_objects(
        &self,
        request: tonic::Request<grpc_state_service::ListOwnedObjectsRequest>,
    ) -> std::result::Result<
        tonic::Response<grpc_state_service::ListOwnedObjectsResponse>,
        tonic::Status,
    > {
        let response =
            list_owned_objects::list_owned_objects(self.reader.clone(), request.into_inner())
                .map(Response::new)
                .map_err(tonic::Status::from)?;
        Ok(append_info_headers!(response, self.reader.clone()))
    }

    async fn get_coin_info(
        &self,
        request: tonic::Request<grpc_state_service::GetCoinInfoRequest>,
    ) -> std::result::Result<tonic::Response<grpc_state_service::GetCoinInfoResponse>, tonic::Status>
    {
        let response = get_coin_info::get_coin_info(self.reader.clone(), request.into_inner())
            .map(Response::new)
            .map_err(tonic::Status::from)?;
        Ok(append_info_headers!(response, self.reader.clone()))
    }
}
