// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use axum::{
    Json,
    extract::{Path, State},
};
use iota_sdk2::types::{ObjectId, StructTag};
use iota_types::iota_sdk2_conversions::struct_tag_sdk_to_core;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    RestError, RestService, Result,
    openapi::{ApiEndpoint, OperationBuilder, ResponseBuilder, RouteHandler},
    reader::StateReader,
};

pub struct GetCoinInfo;

impl ApiEndpoint<RestService> for GetCoinInfo {
    fn method(&self) -> axum::http::Method {
        axum::http::Method::GET
    }

    fn path(&self) -> &'static str {
        "/coins/{coin_type}"
    }

    fn operation(
        &self,
        generator: &mut schemars::gen::SchemaGenerator,
    ) -> openapiv3::v3_1::Operation {
        OperationBuilder::new()
            .tag("Coins")
            .operation_id("GetCoinInfo")
            .path_parameter::<StructTag>("coin_type", generator)
            .response(
                200,
                ResponseBuilder::new()
                    .json_content::<CoinInfo>(generator)
                    .build(),
            )
            .response(404, ResponseBuilder::new().build())
            .build()
    }

    fn handler(&self) -> crate::openapi::RouteHandler<RestService> {
        RouteHandler::new(self.method(), get_coin_info)
    }
}

async fn get_coin_info(
    Path(coin_type): Path<StructTag>,
    State(state): State<StateReader>,
) -> Result<Json<CoinInfo>> {
    let core_coin_type = struct_tag_sdk_to_core(coin_type.clone())?;

    let iota_types::storage::CoinInfo {
        coin_metadata_object_id,
        treasury_object_id,
    } = state
        .inner()
        .get_coin_info(&core_coin_type)?
        .ok_or_else(|| CoinNotFoundError(coin_type.clone()))?;

    let metadata = if let Some(coin_metadata_object_id) = coin_metadata_object_id {
        state
            .inner()
            .get_object(&coin_metadata_object_id)?
            .map(iota_types::coin::CoinMetadata::try_from)
            .transpose()
            .map_err(|_| {
                RestError::new(
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Unable to read object {coin_metadata_object_id} for coin type {core_coin_type} as CoinMetadata"),
                )
            })?
            .map(CoinMetadata::from)
    } else {
        None
    };

    let treasury = if let Some(treasury_object_id) = treasury_object_id {
        state
            .inner()
            .get_object(&treasury_object_id)?
            .map(iota_types::coin::TreasuryCap::try_from)
            .transpose()
            .map_err(|_| {
                RestError::new(
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Unable to read object {treasury_object_id} for coin type {core_coin_type} as TreasuryCap"),
                )
            })?
            .map(|treasury| CoinTreasury {
                id: treasury.id.id.bytes.into(),
                total_supply: treasury.total_supply.value,
            })
    } else if iota_types::gas_coin::GAS::is_gas(&core_coin_type) {
        let system_state_summary = state.get_system_state_summary()?;

        Some(CoinTreasury {
            id: system_state_summary.iota_treasury_cap_id,
            total_supply: system_state_summary.iota_total_supply,
        })
    } else {
        None
    };

    Ok(Json(CoinInfo {
        coin_type,
        metadata,
        treasury,
    }))
}

#[derive(Debug)]
pub struct CoinNotFoundError(StructTag);

impl std::fmt::Display for CoinNotFoundError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Coin type {} not found", self.0)
    }
}

impl std::error::Error for CoinNotFoundError {}

impl From<CoinNotFoundError> for crate::RestError {
    fn from(value: CoinNotFoundError) -> Self {
        Self::new(axum::http::StatusCode::NOT_FOUND, value.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CoinInfo {
    pub coin_type: StructTag,
    pub metadata: Option<CoinMetadata>,
    pub treasury: Option<CoinTreasury>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, JsonSchema)]
pub struct CoinMetadata {
    pub id: ObjectId,
    /// Number of decimal places the coin uses.
    pub decimals: u8,
    /// Name for the token
    pub name: String,
    /// Symbol for the token
    pub symbol: String,
    /// Description of the token
    pub description: String,
    /// URL for the token logo
    pub icon_url: Option<String>,
}

impl From<iota_types::coin::CoinMetadata> for CoinMetadata {
    fn from(value: iota_types::coin::CoinMetadata) -> Self {
        Self {
            id: value.id.id.bytes.into(),
            decimals: value.decimals,
            name: value.name,
            symbol: value.symbol,
            description: value.description,
            icon_url: value.icon_url,
        }
    }
}

#[serde_with::serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, JsonSchema)]
pub struct CoinTreasury {
    pub id: ObjectId,
    #[serde_as(as = "iota_types::iota_serde::BigInt<u64>")]
    #[schemars(with = "crate::_schemars::U64")]
    pub total_supply: u64,
}
