// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use axum::{extract::State, Extension, Json};
use axum_extra::extract::WithRejection;
use iota_json_rpc_types::IotaTransactionBlockResponseOptions;
use tracing::debug;

use crate::{
    types::{
        BlockRequest, BlockResponse, BlockTransactionRequest, BlockTransactionResponse,
        Transaction, TransactionIdentifier,
    },
    Error, OnlineServerContext, IotaEnv,
};

/// This module implements the [Rosetta Block API](https://www.rosetta-api.org/docs/BlockApi.html)

/// Get a block by its Block Identifier.
/// [Rosetta API Spec](https://www.rosetta-api.org/docs/BlockApi.html#block)
pub async fn block(
    State(state): State<OnlineServerContext>,
    Extension(env): Extension<IotaEnv>,
    WithRejection(Json(request), _): WithRejection<Json<BlockRequest>, Error>,
) -> Result<BlockResponse, Error> {
    debug!("Called /block endpoint: {:?}", request.block_identifier);
    env.check_network_identifier(&request.network_identifier)?;
    let blocks = state.blocks();
    if let Some(index) = request.block_identifier.index {
        blocks.get_block_by_index(index).await
    } else if let Some(hash) = request.block_identifier.hash {
        blocks.get_block_by_hash(hash).await
    } else {
        blocks.current_block().await
    }
}

/// Get a transaction in a block by its Transaction Identifier.
/// [Rosetta API Spec](https://www.rosetta-api.org/docs/BlockApi.html#blocktransaction)
pub async fn transaction(
    State(context): State<OnlineServerContext>,
    Extension(env): Extension<IotaEnv>,
    WithRejection(Json(request), _): WithRejection<Json<BlockTransactionRequest>, Error>,
) -> Result<BlockTransactionResponse, Error> {
    env.check_network_identifier(&request.network_identifier)?;
    let digest = request.transaction_identifier.hash;
    let response = context
        .client
        .read_api()
        .get_transaction_with_options(
            digest,
            IotaTransactionBlockResponseOptions::new()
                .with_input()
                .with_events()
                .with_effects()
                .with_balance_changes(),
        )
        .await?;
    let hash = response.digest;

    let operations = response.try_into()?;

    let transaction = Transaction {
        transaction_identifier: TransactionIdentifier { hash },
        operations,
        related_transactions: vec![],
        metadata: None,
    };

    Ok(BlockTransactionResponse { transaction })
}
