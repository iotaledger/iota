// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use axum::{Json, extract::State, response::IntoResponse};
use serde::Serialize;

use crate::types::SharedRestServerAppState;

bin_version::bin_version!();

/// Represent a health status response of the REST API server.
#[derive(Serialize)]
pub struct HealthResponse {
    /// Version of the binary.
    pub version: String,
    /// The Git hash of the binary.
    pub git_hash: String,
    /// The total uptime of the REST API server.
    pub uptime: String,
    /// The status of REST API.
    pub status: String,
}

/// Handles the health check request for the REST API server.
///
/// This endpoint provides information about the server's health, including
/// the version, Git hash and uptime.
pub async fn health(State(app_state): State<SharedRestServerAppState>) -> impl IntoResponse {
    let response = HealthResponse {
        version: VERSION.to_owned(),
        git_hash: GIT_REVISION.to_owned(),
        uptime: format!("{:?}", app_state.kv_store_client.get_uptime()),
        status: "OK".to_owned(),
    };
    Json(response)
}
