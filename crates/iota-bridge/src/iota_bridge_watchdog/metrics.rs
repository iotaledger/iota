// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use prometheus::{
    IntGauge, IntGaugeVec, Registry, register_int_gauge_vec_with_registry,
    register_int_gauge_with_registry,
};

#[derive(Clone, Debug)]
pub struct WatchdogMetrics {
    pub eth_vault_balance: IntGauge,
    pub usdt_vault_balance: IntGauge,
    pub total_supplies: IntGaugeVec,
    pub eth_bridge_paused: IntGauge,
    pub iota_bridge_paused: IntGauge,
}

impl WatchdogMetrics {
    pub fn new(registry: &Registry) -> Self {
        Self {
            eth_vault_balance: register_int_gauge_with_registry!(
                "bridge_eth_vault_balance",
                "Current balance of eth vault",
                registry,
            )
            .unwrap(),
            usdt_vault_balance: register_int_gauge_with_registry!(
                "bridge_usdt_vault_balance",
                "Current balance of usdt eth vault",
                registry,
            )
            .unwrap(),
            total_supplies: register_int_gauge_vec_with_registry!(
                "bridge_total_supplies",
                "Current total supplies of coins on Iota based on Treasury Cap",
                &["token_name"],
                registry,
            )
            .unwrap(),
            eth_bridge_paused: register_int_gauge_with_registry!(
                "bridge_eth_bridge_paused",
                "Whether the eth bridge is paused",
                registry,
            )
            .unwrap(),
            iota_bridge_paused: register_int_gauge_with_registry!(
                "bridge_iota_bridge_paused",
                "Whether the iota bridge is paused",
                registry,
            )
            .unwrap(),
        }
    }

    pub fn new_for_testing() -> Self {
        let registry = Registry::new();
        Self::new(&registry)
    }
}
