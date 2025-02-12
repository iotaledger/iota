// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use axum::{Json, extract::State, response::IntoResponse};
use serde::Serialize;

use crate::{services::AwsStatus, types::SharedKvStoreService};

bin_version::bin_version!();

/// Represent a health status response of the REST API server
#[derive(Serialize)]
pub struct HealthResponse {
    /// The REST API status
    pub status: String,
    /// Version of the binary
    pub version: String,
    /// The Git hash of the binary
    pub git_hash: String,
    /// The total uptime of the REST API server
    pub uptime: String,
    /// The status of AWS components the REST API rely to properly function
    pub aws_status: AwsStatus,
}

pub async fn health(State(kv_store_service): State<SharedKvStoreService>) -> impl IntoResponse {
    let aws_status = kv_store_service.get_aws_health().await;

    let response = HealthResponse {
        status: "ok".to_string(),
        version: VERSION.to_owned(),
        git_hash: GIT_REVISION.to_owned(),
        uptime: format!("{:?}", kv_store_service.get_uptime()),
        aws_status,
    };

    Json(response)
}
