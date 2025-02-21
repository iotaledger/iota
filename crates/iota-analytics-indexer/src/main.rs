// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use clap::*;
use iota_analytics_indexer::{
    AnalyticsIndexerConfig, analytics_metrics::AnalyticsMetrics, errors::AnalyticsIndexerError,
    make_analytics_processor,
};
use iota_data_ingestion_core::{ReaderOptions, setup_single_workflow};
use prometheus::Registry;
use tokio::signal;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    let _guard = telemetry_subscribers::TelemetryConfig::new()
        .with_env()
        .init();

    let config = AnalyticsIndexerConfig::parse();
    info!("Parsed config: {:#?}", config);
    let registry_service = iota_metrics::start_prometheus_server(
        format!(
            "{}:{}",
            config.client_metric_host, config.client_metric_port
        )
        .parse()
        .unwrap(),
    );
    let registry: Registry = registry_service.default_registry();
    iota_metrics::init_metrics(&registry);
    let metrics = AnalyticsMetrics::new(&registry);
    let remote_store_url = config.remote_store_url.clone();
    let processor = make_analytics_processor(config, metrics)
        .await
        .map_err(|e| AnalyticsIndexerError::Generic(e.to_string()))?;
    let watermark = processor.last_committed_checkpoint().unwrap_or_default() + 1;

    let reader_options = ReaderOptions {
        batch_size: 10,
        ..Default::default()
    };
    let (executor, token) = setup_single_workflow(
        processor,
        remote_store_url,
        watermark,
        1,
        Some(reader_options),
    )
    .await?;

    tokio::spawn(async move {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
        token.cancel();
    });
    executor.await?;
    Ok(())
}
