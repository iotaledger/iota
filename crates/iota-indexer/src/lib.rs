// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#![recursion_limit = "256"]

use std::time::Duration;

use anyhow::Result;
use errors::IndexerError;
use iota_grpc_client::Client as GrpcClient;
use iota_json_rpc::{JsonRpcServerBuilder, ServerHandle, ServerType};
use iota_metrics::spawn_monitored_task;
use metrics::IndexerMetrics;
use prometheus::Registry;
use system_package_task::SystemPackageTask;
use tokio_util::sync::CancellationToken;

use crate::{
    apis::{
        CoinReadApi, ExtendedApi, GovernanceReadApi, IndexerApi, MoveUtilsApi, OptimisticWriteApi,
        ReadApi, TransactionBuilderApi, WriteApi,
    },
    config::JsonRpcConfig,
    optimistic_indexing::OptimisticTransactionExecutor,
    read::IndexerReader,
    store::PgIndexerStore,
};

pub mod apis;
pub mod backfill;
pub mod config;
pub mod db;
pub mod errors;
pub mod historical_fallback;
pub mod indexer;
pub mod ingestion;
pub mod metrics;
pub mod models;
pub mod optimistic_indexing;
pub mod processors;
pub mod pruning;
pub mod read;
pub mod schema;
pub mod store;
pub mod system_package_task;
pub mod test_utils;
pub mod types;

pub async fn build_json_rpc_server(
    store: PgIndexerStore,
    prometheus_registry: &Registry,
    reader: IndexerReader,
    config: &JsonRpcConfig,
    metrics: IndexerMetrics,
) -> Result<ServerHandle, IndexerError> {
    let mut builder =
        JsonRpcServerBuilder::new(env!("CARGO_PKG_VERSION"), prometheus_registry, None, None);

    let fullnode_grpc_client = GrpcClient::connect(&config.rpc_client_url).await?;
    // Register common modules
    builder.register_module(IndexerApi::new(
        reader.clone(),
        config.iota_names_options.clone().into(),
    ))?;
    builder.register_module(TransactionBuilderApi::from(reader.clone()))?;
    builder.register_module(MoveUtilsApi::new(reader.clone()))?;
    builder.register_module(GovernanceReadApi::new(reader.clone()))?;
    builder.register_module(ReadApi::new(reader.clone(), fullnode_grpc_client.clone()))?;
    builder.register_module(CoinReadApi::new(reader.clone())?)?;
    builder.register_module(ExtendedApi::new(reader.clone()))?;
    builder.register_module(OptimisticWriteApi::new(
        WriteApi::new(fullnode_grpc_client.clone(), reader.clone()),
        OptimisticTransactionExecutor::new(fullnode_grpc_client, reader.clone(), store, metrics)
            .await?,
    ))?;

    let cancel = CancellationToken::new();
    let system_package_task =
        SystemPackageTask::new(reader, cancel.clone(), Duration::from_secs(10));

    tracing::info!("Starting system package task");
    spawn_monitored_task!(async move { system_package_task.run().await });

    Ok(builder
        .start(config.rpc_address, None, ServerType::Http, Some(cancel))
        .await?)
}
