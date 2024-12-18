// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use std::env;
use std::sync::Arc;
use iota_config::iota_config_dir;
use iota_faucet::{create_wallet_context, start_faucet, AppState};
use iota_faucet::{FaucetConfig, SimpleFaucet};
use tracing::info;

const CONCURRENCY_LIMIT: usize = 30;
const PROM_PORT_ADDR: &str = "0.0.0.0:9184";

// Define the `GIT_REVISION` and `VERSION` consts
bin_version::bin_version!();

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // initialize tracing
    let _guard = telemetry_subscribers::TelemetryConfig::new()
        .with_env()
        .init();

    let config: FaucetConfig = FaucetConfig::parse();
    let FaucetConfig {
        wallet_client_timeout_secs,
        ref write_ahead_log,
        ..
    } = config;

    let context = create_wallet_context(wallet_client_timeout_secs, iota_config_dir()?)?;

    let max_concurrency = match env::var("MAX_CONCURRENCY") {
        Ok(val) => val.parse::<usize>().unwrap(),
        _ => CONCURRENCY_LIMIT,
    };
    info!("Max concurrency: {max_concurrency}.");

    let prom_binding = PROM_PORT_ADDR.parse().unwrap();
    info!("Starting Prometheus HTTP endpoint at {}", prom_binding);
    let registry_service = iota_metrics::start_prometheus_server(prom_binding);
    let prometheus_registry = registry_service.default_registry();
    prometheus_registry
        .register(iota_metrics::uptime_metric("faucet", VERSION, "unknown"))
        .unwrap();

    let app_state = Arc::new(AppState {
        faucet: SimpleFaucet::new(
            context,
            &prometheus_registry,
            write_ahead_log,
            config.clone(),
        )
        .await
        .unwrap(),
        config,
    });

    start_faucet(app_state, max_concurrency, &prometheus_registry).await
}
