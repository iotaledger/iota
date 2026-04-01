// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! REST API module for Simulacrum Server
//!
//! This module provides REST endpoints for controlling and querying the
//! simulacrum.

use std::{borrow::Cow, sync::Arc, time::Duration};

use axum::{
    BoxError, Extension, Json, Router,
    error_handling::HandleErrorLayer,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use http::Method;
use iota_types::storage::ReadStore;
use serde::{Deserialize, Serialize};
use simulacrum::Simulacrum;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use crate::faucet;

/// Application state shared between REST handlers
#[derive(Clone)]
pub struct AppState {
    pub simulacrum: Arc<Simulacrum>,
    pub faucet_request_amount: u64,
    pub chain_id: String,
}

/// REST API response types
#[derive(Serialize)]
pub struct SimulacrumStatus {
    pub chain_id: String,
    pub current_epoch: u64,
    pub highest_checkpoint: Option<u64>,
    pub timestamp_ms: u64,
    pub reference_gas_price: u64,
}

#[derive(Debug, Serialize)]
pub struct CheckpointResponse {
    pub sequence_number: u64,
    pub epoch: u64,
    pub timestamp_ms: u64,
    pub network_total_transactions: u64,
    pub content_digest: String,
}

#[derive(Deserialize)]
pub struct AdvanceClockRequest {
    pub duration_ms: u64,
}

#[derive(Deserialize)]
pub struct AdvanceEpochRequest {
    #[serde(default)]
    pub create_checkpoint: bool,
}

#[derive(Deserialize)]
pub struct CreateCheckpointsRequest {
    pub count: u64,
    #[serde(default = "default_interval_ms")]
    pub interval_ms: u64,
}

fn default_interval_ms() -> u64 {
    1000 // 1 second default
}

#[derive(Serialize)]
pub struct AdvanceClockResponse {
    pub new_timestamp_ms: u64,
    pub advanced_by_ms: u64,
}

#[derive(Serialize)]
pub struct AdvanceEpochResponse {
    pub new_epoch: u64,
    pub checkpoint: Option<CheckpointResponse>,
}

#[derive(Serialize)]
pub struct CreateCheckpointsResponse {
    pub created_count: u64,
    pub checkpoints: Vec<CheckpointResponse>,
}

/// Helper function to convert a checkpoint to response format
pub fn checkpoint_to_response(
    checkpoint: &iota_types::messages_checkpoint::VerifiedCheckpoint,
) -> CheckpointResponse {
    let checkpoint_data = checkpoint.data();
    CheckpointResponse {
        sequence_number: *checkpoint.sequence_number(),
        epoch: checkpoint.epoch(),
        timestamp_ms: checkpoint_data.timestamp_ms,
        network_total_transactions: checkpoint_data.network_total_transactions,
        content_digest: checkpoint_data.content_digest.to_string(),
    }
}

/// REST API handlers
pub async fn get_status(
    State(state): State<AppState>,
) -> Result<Json<SimulacrumStatus>, StatusCode> {
    let simulacrum = state.simulacrum.as_ref();

    let highest_verified_checkpoint = simulacrum.get_highest_verified_checkpoint();
    let highest_verified_checkpoint_data = highest_verified_checkpoint.data();
    let highest_checkpoint = Some(*highest_verified_checkpoint_data.sequence_number());
    let current_epoch = highest_verified_checkpoint_data.epoch;

    let timestamp_ms = simulacrum.with_store(|store| store.get_clock().timestamp_ms());
    let reference_gas_price = simulacrum.reference_gas_price();

    let response = SimulacrumStatus {
        chain_id: state.chain_id.clone(),
        current_epoch,
        highest_checkpoint,
        timestamp_ms,
        reference_gas_price,
    };

    Ok(Json(response))
}

pub async fn get_checkpoint(
    State(state): State<AppState>,
) -> Result<Json<CheckpointResponse>, StatusCode> {
    let simulacrum = state.simulacrum.as_ref();
    let highest_verified_checkpoint = simulacrum.get_highest_verified_checkpoint();

    let response = checkpoint_to_response(&highest_verified_checkpoint);
    Ok(Json(response))
}

pub async fn create_checkpoint(
    State(state): State<AppState>,
) -> Result<Json<CheckpointResponse>, StatusCode> {
    let simulacrum = state.simulacrum.as_ref();
    let checkpoint = simulacrum.create_checkpoint();

    let response = checkpoint_to_response(&checkpoint);

    info!(
        "Created checkpoint {} in epoch {} with {} transactions",
        checkpoint.sequence_number(),
        checkpoint.epoch(),
        checkpoint.data().network_total_transactions
    );

    Ok(Json(response))
}

pub async fn create_checkpoints(
    State(state): State<AppState>,
    Json(request): Json<CreateCheckpointsRequest>,
) -> Result<Json<CreateCheckpointsResponse>, StatusCode> {
    if request.count == 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let simulacrum = state.simulacrum.as_ref();
    let mut checkpoints = Vec::new();

    for i in 0..request.count {
        if i > 0 {
            // Advance clock between checkpoints
            simulacrum.advance_clock(Duration::from_millis(request.interval_ms));
        }

        let checkpoint = simulacrum.create_checkpoint();
        checkpoints.push(checkpoint_to_response(&checkpoint));
    }

    info!(
        "Created {} checkpoints with {}ms intervals",
        request.count, request.interval_ms
    );

    let response = CreateCheckpointsResponse {
        created_count: request.count,
        checkpoints,
    };

    Ok(Json(response))
}

pub async fn advance_clock(
    State(state): State<AppState>,
    Json(request): Json<AdvanceClockRequest>,
) -> Result<Json<AdvanceClockResponse>, StatusCode> {
    let simulacrum = state.simulacrum.as_ref();

    let old_timestamp = simulacrum.with_store(|store| store.get_clock().timestamp_ms());
    simulacrum.advance_clock(Duration::from_millis(request.duration_ms));
    let new_timestamp = simulacrum.with_store(|store| store.get_clock().timestamp_ms());

    let response = AdvanceClockResponse {
        new_timestamp_ms: new_timestamp,
        advanced_by_ms: new_timestamp - old_timestamp,
    };

    info!(
        "Advanced clock by {} ms, new timestamp: {} ms",
        response.advanced_by_ms, response.new_timestamp_ms
    );

    Ok(Json(response))
}

pub async fn advance_epoch(
    State(state): State<AppState>,
    Json(request): Json<AdvanceEpochRequest>,
) -> Result<Json<AdvanceEpochResponse>, StatusCode> {
    let simulacrum = state.simulacrum.as_ref();

    let old_epoch = simulacrum.get_highest_verified_checkpoint().data().epoch;

    simulacrum.advance_epoch();

    let new_epoch = simulacrum.get_highest_verified_checkpoint().data().epoch;

    let checkpoint = if request.create_checkpoint {
        let cp = simulacrum.create_checkpoint();
        Some(checkpoint_to_response(&cp))
    } else {
        None
    };

    let response = AdvanceEpochResponse {
        new_epoch,
        checkpoint,
    };

    info!(
        "Advanced epoch from {} to {}, created checkpoint: {}",
        old_epoch, new_epoch, request.create_checkpoint
    );

    Ok(Json(response))
}

async fn handle_error(error: BoxError) -> impl IntoResponse {
    if error.is::<tower::load_shed::error::Overloaded>() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Cow::from("service is overloaded, please try again later"),
        );
    }

    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Cow::from(format!("Unhandled internal error: {error}")),
    )
}

/// Create the REST API router
pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_methods(vec![Method::GET, Method::POST])
        .allow_headers(Any)
        .allow_origin(Any);

    // Configuration values
    let request_buffer_size = 1000;
    let concurrency_limit = 1;

    Router::new()
        .route("/status", get(get_status))
        .route("/checkpoint", get(get_checkpoint))
        .route("/checkpoint/create", post(create_checkpoint))
        .route("/checkpoint/create_multiple", post(create_checkpoints))
        .route("/clock/advance", post(advance_clock))
        .route("/epoch/advance", post(advance_epoch))
        .nest("/faucet", faucet::add_faucet_routes())
        .with_state(state.clone())
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(handle_error))
                .layer(cors)
                .load_shed()
                .buffer(request_buffer_size)
                .concurrency_limit(concurrency_limit)
                .layer(Extension(state))
                .into_inner(),
        )
}
