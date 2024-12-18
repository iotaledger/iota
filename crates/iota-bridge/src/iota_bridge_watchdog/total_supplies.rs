// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The IotaBridgeStatus observable monitors whether the Iota Bridge is paused.

use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use iota_sdk::IotaClient;
use prometheus::IntGaugeVec;
use tokio::time::Duration;
use tracing::{error, info};

use crate::iota_bridge_watchdog::Observable;

pub struct TotalSupplies {
    iota_client: Arc<IotaClient>,
    coins: BTreeMap<String, String>,
    metric: IntGaugeVec,
}

impl TotalSupplies {
    pub fn new(
        iota_client: Arc<IotaClient>,
        coins: BTreeMap<String, String>,
        metric: IntGaugeVec,
    ) -> Self {
        Self {
            iota_client,
            coins,
            metric,
        }
    }
}

#[async_trait]
impl Observable for TotalSupplies {
    fn name(&self) -> &str {
        "TotalSupplies"
    }

    async fn observe_and_report(&self) {
        for (coin_name, coin_type) in &self.coins {
            let resp = self
                .iota_client
                .coin_read_api()
                .get_total_supply(coin_type.clone())
                .await;
            match resp {
                Ok(supply) => {
                    self.metric
                        .with_label_values(&[coin_name])
                        .set(supply.value as i64);
                    info!("Total supply for {coin_type}: {}", supply.value);
                }
                Err(e) => {
                    error!("Error getting total supply for coin {coin_type}: {:?}", e);
                }
            }
        }
    }

    fn interval(&self) -> Duration {
        Duration::from_secs(10)
    }
}
