// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The IotaBridgeStatus observable monitors whether the Iota Bridge is paused.

use crate::iota_bridge_watchdog::Observable;
use crate::iota_client::IotaBridgeClient;
use async_trait::async_trait;
use prometheus::IntGauge;
use std::sync::Arc;

use tokio::time::Duration;
use tracing::{error, info};

pub struct IotaBridgeStatus {
    iota_client: Arc<IotaBridgeClient>,
    metric: IntGauge,
}

impl IotaBridgeStatus {
    pub fn new(iota_client: Arc<IotaBridgeClient>, metric: IntGauge) -> Self {
        Self { iota_client, metric }
    }
}

#[async_trait]
impl Observable for IotaBridgeStatus {
    fn name(&self) -> &str {
        "IotaBridgeStatus"
    }

    async fn observe_and_report(&self) {
        let status = self.iota_client.is_bridge_paused().await;
        match status {
            Ok(status) => {
                self.metric.set(status as i64);
                info!("Iota Bridge Status: {:?}", status);
            }
            Err(e) => {
                error!("Error getting iota bridge status: {:?}", e);
            }
        }
    }

    fn interval(&self) -> Duration {
        Duration::from_secs(10)
    }
}
