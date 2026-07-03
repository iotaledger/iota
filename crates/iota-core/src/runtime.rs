// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_config::NodeConfig;
use tokio::runtime::Runtime;

pub struct IotaRuntimes {
    // Order in this struct is the order in which runtimes are stopped.
    /// Client-facing servers (validator gRPC, JSON-RPC, gRPC read API) and the
    /// per-request handlers they spawn. Isolated from `iota_node` so that a
    /// flood of external requests cannot starve the node core. Stopped first so
    /// that client requests are cut off before the node core they call into is
    /// torn down.
    pub serving: Runtime,
    /// Node core: consensus, execution, state sync, checkpoints and p2p
    /// networking.
    pub iota_node: Runtime,
    pub metrics: Runtime,
}

impl IotaRuntimes {
    pub fn new(config: &NodeConfig) -> Self {
        let available = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(8);
        // Split worker threads between the node core and the serving runtime
        // according to the node's role. A validator runs consensus and execution
        // on the core and must protect it from client load, so it reserves most
        // threads for the core. A fullnode has no consensus and exists primarily
        // to serve reads, so it favors the serving runtime. The split is sized so
        // the two runtimes together do not oversubscribe the available cores.
        let is_validator = config.consensus_config().is_some();
        let serving_threads = if is_validator {
            (available / 4).max(2)
        } else {
            (available / 2).max(2)
        };
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
            serving,
            iota_node,
            metrics,
        }
    }
}
