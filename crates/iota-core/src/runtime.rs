// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_config::NodeConfig;
use tokio::runtime::Runtime;

pub struct IotaRuntimes {
    // Order in this struct is the order in which runtimes are stopped.
    /// Node core: consensus, execution, state sync, checkpoints and p2p
    /// networking.
    pub iota_node: Runtime,
    /// Client-facing servers (validator gRPC, JSON-RPC, gRPC read API) and the
    /// per-request handlers they spawn. Isolated from `iota_node` so that a
    /// flood of external requests cannot starve the node core.
    pub serving: Runtime,
    pub metrics: Runtime,
}

impl IotaRuntimes {
    pub fn new(_config: &NodeConfig) -> Self {
        let available = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(8);
        // Reserve half the cores for serving and keep the rest for the node core,
        // so the two runtimes do not oversubscribe physical cores.
        let serving_threads = (available / 2).max(2);
        let node_threads = available.saturating_sub(serving_threads).max(4);

        let iota_node = tokio::runtime::Builder::new_multi_thread()
            .thread_name("iota-node-runtime")
            .worker_threads(node_threads)
            .enable_all()
            .build()
            .unwrap();
        let serving = tokio::runtime::Builder::new_multi_thread()
            .thread_name("serving-runtime")
            .worker_threads(serving_threads)
            .enable_all()
            .build()
            .unwrap();
        let metrics = tokio::runtime::Builder::new_multi_thread()
            .thread_name("metrics-runtime")
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();

        Self {
            iota_node,
            serving,
            metrics,
        }
    }
}
