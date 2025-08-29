// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use clap::Parser;
use iota_synthetic_ingestion::synthetic_ingestion::{Config, generate_ingestion};

#[tokio::main]
async fn main() -> Result<()> {
    let _guard = telemetry_subscribers::TelemetryConfig::new()
        .with_env()
        .init();

    let config = Config::parse();
    generate_ingestion(config).await
}
