// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_config::{NodeConfig, node::available_cpu_cores};
use tokio::runtime::Runtime;

/// Minimum number of worker threads for the serving runtime.
const MIN_SERVING_THREADS: usize = 2;
/// Minimum number of worker threads for the node-core runtime.
const MIN_NODE_THREADS: usize = 4;

/// Splits the available cores between the node-core and serving runtimes,
/// returning `(node_threads, serving_threads)`.
///
/// A validator runs consensus and execution on the core and must protect it
/// from client load, so it reserves most threads for the core. A fullnode has
/// no consensus and exists primarily to serve reads, so it splits threads
/// evenly. The two runtimes together never exceed `available`, except on
/// machines with fewer than `MIN_NODE_THREADS + MIN_SERVING_THREADS` cores,
/// where the minimum floors win.
fn split_worker_threads(available: usize, is_validator: bool) -> (usize, usize) {
    let serving_share = if is_validator {
        available / 4
    } else {
        available / 2
    }
    .max(MIN_SERVING_THREADS);
    // The core has priority: it gets everything the serving share does not use,
    // but never fewer than its minimum. Serving then gets whatever is actually
    // left, so the floors are the only way the split can exceed `available`.
    let node_threads = available
        .saturating_sub(serving_share)
        .max(MIN_NODE_THREADS);
    let serving_threads = available
        .saturating_sub(node_threads)
        .max(MIN_SERVING_THREADS);
    (node_threads, serving_threads)
}

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
        let is_validator = config.consensus_config().is_some();
        let (node_threads, serving_threads) =
            split_worker_threads(available_cpu_cores(), is_validator);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validator_reserves_most_threads_for_the_core() {
        assert_eq!(split_worker_threads(16, true), (12, 4));
        assert_eq!(split_worker_threads(8, true), (6, 2));
    }

    #[test]
    fn fullnode_splits_threads_evenly() {
        assert_eq!(split_worker_threads(16, false), (8, 8));
        assert_eq!(split_worker_threads(12, false), (6, 6));
    }

    #[test]
    fn split_never_oversubscribes_above_the_minimum_floors() {
        for available in (MIN_NODE_THREADS + MIN_SERVING_THREADS)..=256 {
            for is_validator in [true, false] {
                let (node, serving) = split_worker_threads(available, is_validator);
                assert_eq!(
                    node + serving,
                    available,
                    "oversubscribed with {available} cores (is_validator: {is_validator}): \
                     node {node} + serving {serving}",
                );
            }
        }
    }

    #[test]
    fn small_machines_fall_back_to_the_minimum_floors() {
        for available in 1..(MIN_NODE_THREADS + MIN_SERVING_THREADS) {
            for is_validator in [true, false] {
                assert_eq!(
                    split_worker_threads(available, is_validator),
                    (MIN_NODE_THREADS, MIN_SERVING_THREADS),
                );
            }
        }
    }

    #[test]
    fn both_runtimes_always_get_their_minimum_threads() {
        for available in 1..=256 {
            for is_validator in [true, false] {
                let (node, serving) = split_worker_threads(available, is_validator);
                assert!(node >= MIN_NODE_THREADS);
                assert!(serving >= MIN_SERVING_THREADS);
            }
        }
    }
}
