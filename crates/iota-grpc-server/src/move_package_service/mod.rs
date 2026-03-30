// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod list_package_versions;

use std::sync::Arc;

use iota_grpc_types::v1::move_package_service::{self as grpc_move_package_service};
use tonic::Response;

use crate::types::*;

pub struct MovePackageGrpcService {
    pub reader: Arc<GrpcReader>,
}

impl MovePackageGrpcService {
    pub fn new(reader: Arc<GrpcReader>) -> Self {
        Self { reader }
    }
}

#[tonic::async_trait]
impl grpc_move_package_service::move_package_service_server::MovePackageService
    for MovePackageGrpcService
{
    async fn list_package_versions(
        &self,
        request: tonic::Request<grpc_move_package_service::ListPackageVersionsRequest>,
    ) -> std::result::Result<
        tonic::Response<grpc_move_package_service::ListPackageVersionsResponse>,
        tonic::Status,
    > {
        let response =
            list_package_versions::list_package_versions(self.reader.clone(), request.into_inner())
                .map(Response::new)
                .map_err(tonic::Status::from)?;
        Ok(append_info_headers!(response, self.reader.clone()))
    }
}
