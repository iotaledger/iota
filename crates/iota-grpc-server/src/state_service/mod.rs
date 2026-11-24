// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod get_coin_info;
mod list_dynamic_fields;
mod list_owned_objects;

use std::sync::Arc;

use iota_config::node::GrpcApiConfig;
use iota_grpc_types::v1::state_service::{self as grpc_state_service};
use tonic::{Code, Response};

use crate::{metrics::RequestMetrics, traffic_control::TallyHandle, types::*};

/// Charge a unary batch read to the traffic controller: one request and one
/// response item per returned item, plus `rendered_fields_per_item` rendered
/// fields each, so a large or expensive response draws down its own dimension
/// budgets. An empty response is charged as one request with no items, matching
/// the streaming reads. Calling this marks the request accounted, so the
/// traffic-control layer skips its default per-request tally.
fn tally_read_items(
    tally_handle: Option<&TallyHandle>,
    count: usize,
    rendered_fields_per_item: u32,
) {
    let Some(tally_handle) = tally_handle else {
        return;
    };
    if count == 0 {
        tally_handle.tally_item(Code::Ok);
    } else {
        tally_handle.tally_response_items(count, rendered_fields_per_item);
    }
}

pub struct StateGrpcService {
    pub config: GrpcApiConfig,
    pub reader: Arc<GrpcReader>,
}

impl StateGrpcService {
    pub fn new(config: GrpcApiConfig, reader: Arc<GrpcReader>) -> Self {
        Self { config, reader }
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
        let tally_handle = request.extensions().get::<TallyHandle>().cloned();
        let metrics = RequestMetrics::from_extensions(request.extensions());
        let request = request.into_inner();
        let rendered_fields =
            crate::utils::dynamic_field_rendered_field_weight(request.read_mask.as_ref()) as u32;
        let response = list_dynamic_fields::list_dynamic_fields(
            self.reader.clone(),
            self.config.max_json_move_value_size,
            &metrics,
            request,
        )
        .map_err(tonic::Status::from)?;
        tally_read_items(
            tally_handle.as_ref(),
            response.dynamic_fields.len(),
            rendered_fields,
        );
        Ok(append_info_headers!(
            Response::new(response),
            self.reader.clone()
        ))
    }

    async fn list_owned_objects(
        &self,
        request: tonic::Request<grpc_state_service::ListOwnedObjectsRequest>,
    ) -> std::result::Result<
        tonic::Response<grpc_state_service::ListOwnedObjectsResponse>,
        tonic::Status,
    > {
        let tally_handle = request.extensions().get::<TallyHandle>().cloned();
        let metrics = RequestMetrics::from_extensions(request.extensions());
        let request = request.into_inner();
        let rendered_fields =
            crate::utils::object_rendered_field_weight(request.read_mask.as_ref()) as u32;
        let response = list_owned_objects::list_owned_objects(
            self.reader.clone(),
            self.config.max_json_move_value_size,
            &metrics,
            request,
        )
        .map_err(tonic::Status::from)?;
        tally_read_items(
            tally_handle.as_ref(),
            response.objects.len(),
            rendered_fields,
        );
        Ok(append_info_headers!(
            Response::new(response),
            self.reader.clone()
        ))
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
