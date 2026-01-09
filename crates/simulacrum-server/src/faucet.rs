// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Faucet module for Simulacrum Server
//!
//! This module provides faucet functionality for requesting gas tokens,
//! using the same types and endpoints as the iota-faucet crate.

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
};
use iota_faucet::{
    BatchFaucetResponse, CoinInfo, FaucetError, FaucetReceipt, FaucetRequest, FaucetResponse,
};
use iota_types::effects::TransactionEffectsAPI;
use tracing::{info, warn};
use uuid::Uuid;

use crate::rest_api::AppState;

/// Health check endpoint for faucet
pub async fn health() -> Result<Json<&'static str>, StatusCode> {
    Ok(Json("OK"))
}

/// Internal function that handles the actual gas request logic
async fn request_gas_internal(
    state: &AppState,
    recipient: iota_types::base_types::IotaAddress,
) -> Result<FaucetReceipt, String> {
    let amount = state.faucet_request_amount;

    info!(
        "Faucet request for {} NANOS to address: {}",
        amount, recipient
    );

    let simulacrum = state.simulacrum.as_ref();

    // Request gas from the faucet
    let gas_result = simulacrum.request_gas(recipient, amount);

    match gas_result {
        Ok(effects) => {
            info!(
                "Gas request successful, effects: {:?}",
                effects.summary_for_debug()
            );

            // Create a checkpoint to finalize the transaction
            let checkpoint = simulacrum.create_checkpoint();

            // Extract created gas objects from effects
            let mut sent_coins = Vec::new();
            for (obj_ref, _) in effects.created().iter() {
                sent_coins.push(CoinInfo {
                    amount,
                    id: obj_ref.0, // Extract ObjectID from ObjectRef tuple
                    transfer_tx_digest: *effects.transaction_digest(),
                });
            }

            let receipt = FaucetReceipt { sent: sent_coins };

            info!(
                "Faucet request completed successfully for address: {}, checkpoint: {}",
                recipient,
                checkpoint.sequence_number()
            );

            Ok(receipt)
        }
        Err(err) => {
            warn!("Failed to request gas: {:?}", err);
            Err(format!("Failed to request gas: {}", err))
        }
    }
}

/// Main faucet gas request handler matching iota-faucet endpoints
pub async fn request_gas(
    State(state): State<AppState>,
    Json(payload): Json<FaucetRequest>,
) -> impl IntoResponse {
    match payload {
        FaucetRequest::FixedAmountRequest(request) => {
            match request_gas_internal(&state, request.recipient).await {
                Ok(receipt) => (StatusCode::CREATED, Json(FaucetResponse::from(receipt))),
                Err(err) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(FaucetResponse {
                        error: Some(err),
                        transferred_gas_objects: vec![],
                    }),
                ),
            }
        }
        FaucetRequest::GetBatchSendStatusRequest(_) => {
            // Batch operations not supported in simulacrum
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(FaucetResponse {
                    error: Some("Batch operations not supported in simulacrum".to_string()),
                    transferred_gas_objects: vec![],
                }),
            )
        }
    }
}

pub async fn batch_request_gas(
    State(state): State<AppState>,
    Json(payload): Json<FaucetRequest>,
) -> impl IntoResponse {
    let FaucetRequest::FixedAmountRequest(request) = payload else {
        return (
            StatusCode::BAD_REQUEST,
            Json(BatchFaucetResponse::from(FaucetError::Internal(
                "Input Error.".to_string(),
            ))),
        );
    };

    match request_gas_internal(&state, request.recipient).await {
        Ok(_receipt) => (
            StatusCode::CREATED,
            Json(BatchFaucetResponse {
                task: Some(Uuid::new_v4().to_string()),
                error: None,
            }),
        ),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(BatchFaucetResponse::from(FaucetError::Internal(err))),
        ),
    }
}

/// Add faucet routes matching iota-faucet endpoints
pub fn add_faucet_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(health))
        .route("/gas", post(request_gas))
        .route("/v1/gas", post(batch_request_gas))
}
