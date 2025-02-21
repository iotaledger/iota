// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This module implements the [Rosetta Network API](https://www.rosetta-api.org/docs/NetworkApi.html).

use axum::{Extension, Json, extract::State};
use axum_extra::extract::WithRejection;
use fastcrypto::encoding::Hex;
use iota_types::base_types::ObjectID;
use serde_json::json;
use strum::IntoEnumIterator;

use crate::{
    IotaEnv, OnlineServerContext,
    errors::{Error, ErrorType},
    types::{
        Allow, Case, NetworkIdentifier, NetworkListResponse, NetworkOptionsResponse,
        NetworkRequest, NetworkStatusResponse, OperationStatus, OperationType, Peer, SyncStatus,
        Version,
    },
};

/// This endpoint returns a list of NetworkIdentifiers that the Rosetta server
/// supports.
///
/// [Rosetta API Spec](https://www.rosetta-api.org/docs/NetworkApi.html#networklist)
pub async fn list(Extension(env): Extension<IotaEnv>) -> Result<NetworkListResponse, Error> {
    Ok(NetworkListResponse {
        network_identifiers: vec![NetworkIdentifier {
            blockchain: "iota".to_string(),
            network: env,
        }],
    })
}

/// This endpoint returns the current status of the network requested.
///
/// [Rosetta API Spec](https://www.rosetta-api.org/docs/NetworkApi.html#networkstatus)
pub async fn status(
    State(context): State<OnlineServerContext>,
    Extension(env): Extension<IotaEnv>,
    WithRejection(Json(request), _): WithRejection<Json<NetworkRequest>, Error>,
) -> Result<NetworkStatusResponse, Error> {
    env.check_network_identifier(&request.network_identifier)?;

    let system_state = context
        .client
        .governance_api()
        .get_latest_iota_system_state()
        .await?;

    let peers = system_state
        .active_validators
        .iter()
        .map(|validator| Peer {
            peer_id: ObjectID::from(validator.iota_address).into(),
            metadata: Some(json!({
                "public_key": Hex::from_bytes(&validator.authority_pubkey_bytes),
                "stake_amount": validator.staking_pool_iota_balance,
            })),
        })
        .collect();
    let blocks = context.blocks();
    let current_block = blocks.current_block().await?;
    let index = current_block.block.block_identifier.index;
    let target = context
        .client
        .read_api()
        .get_latest_checkpoint_sequence_number()
        .await?;

    Ok(NetworkStatusResponse {
        current_block_identifier: current_block.block.block_identifier,
        current_block_timestamp: current_block.block.timestamp,
        genesis_block_identifier: blocks.genesis_block_identifier().await?,
        oldest_block_identifier: Some(blocks.oldest_block_identifier().await?),
        sync_status: Some(SyncStatus {
            current_index: Some(index),
            target_index: Some(target),
            stage: None,
            synced: Some(index == target),
        }),
        peers,
    })
}

/// This endpoint returns the version information and allowed network-specific
/// types for a NetworkIdentifier.
///
/// [Rosetta API Spec](https://www.rosetta-api.org/docs/NetworkApi.html#networkoptions)
pub async fn options(
    Extension(env): Extension<IotaEnv>,
    WithRejection(Json(request), _): WithRejection<Json<NetworkRequest>, Error>,
) -> Result<NetworkOptionsResponse, Error> {
    env.check_network_identifier(&request.network_identifier)?;

    let errors = ErrorType::iter().collect();
    let operation_statuses = vec![
        json!({"status": OperationStatus::Success, "successful" : true}),
        json!({"status": OperationStatus::Failure, "successful" : false}),
    ];

    Ok(NetworkOptionsResponse {
        version: Version {
            rosetta_version: "1.4.14".to_string(),
            node_version: env!("CARGO_PKG_VERSION").to_owned(),
            middleware_version: None,
            metadata: None,
        },
        allow: Allow {
            operation_statuses,
            operation_types: OperationType::iter().collect(),
            errors,
            historical_balance_lookup: true,
            timestamp_start_index: None,
            call_methods: vec![],
            balance_exemptions: vec![],
            mempool_coins: false,
            block_hash_case: Some(Case::Null),
            transaction_hash_case: Some(Case::Null),
        },
    })
}
