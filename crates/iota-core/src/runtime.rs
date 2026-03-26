// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_config::NodeConfig;
use tokio::runtime::Runtime;

pub struct IotaRuntimes {
    // Order in this struct is the order in which runtimes are stopped
    pub iota_node: Runtime,
    pub metrics: Runtime,
}

impl IotaRuntimes {
    pub fn new(_config: &NodeConfig) -> Self {
        let iota_node = tokio::runtime::Builder::new_multi_thread()
            .thread_name("iota-node-runtime")
            .enable_all()
            .build()
            .unwrap();
        let metrics = tokio::runtime::Builder::new_multi_thread()
            .thread_name("metrics-runtime")
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();

        Self { iota_node, metrics }
    }
}
