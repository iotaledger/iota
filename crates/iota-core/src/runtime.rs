// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_config::NodeConfig;
use tokio::runtime::Runtime;

/// Minimum number of worker threads for the serving runtime.
const MIN_SERVING_THREADS: usize = 2;
/// Minimum number of worker threads for the node-core runtime.
const MIN_NODE_THREADS: usize = 4;
/// Number of worker threads for the metrics runtime. Small and mostly idle;
/// sits outside [`size_worker_threads`].
const METRICS_THREADS: usize = 2;

/// Number of CPU cores available to the process, falling back to 8 when it
/// cannot be determined.
pub fn available_cpu_cores() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8)
}

/// Sizes the node-core and serving runtimes, returning `(node_threads,
/// serving_threads)`.
///
/// The core always gets one worker per available core, so it can use the
/// whole machine while serving is idle. The serving workers are added on top,
/// deliberately oversubscribing the machine: a worker blocked in RocksDB
/// frees its core for the other runtime, and when both runtimes are saturated
/// the OS schedules fairly per thread, so the serving pool's size relative to
/// the core pool bounds the CPU share serving can take. A validator caps
/// serving at a quarter of the cores (at most ~20% of the machine under
/// saturation) to protect consensus and execution; a fullnode, whose primary
/// job is serving reads, allows half (~33%).
fn size_worker_threads(available: usize, is_validator: bool) -> (usize, usize) {
    let node_threads = available.max(MIN_NODE_THREADS);
    let serving_threads = if is_validator {
        available / 4
    } else {
        available / 2
    }
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
            size_worker_threads(available_cpu_cores(), is_validator);

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
            .worker_threads(METRICS_THREADS)
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
    fn available_cpu_cores_is_never_zero() {
        assert!(super::available_cpu_cores() >= 1);
    }

    #[test]
    fn validator_serving_gets_a_quarter_of_the_cores() {
        assert_eq!(size_worker_threads(16, true), (16, 4));
        assert_eq!(size_worker_threads(8, true), (8, 2));
    }

    #[test]
    fn fullnode_serving_gets_half_of_the_cores() {
        assert_eq!(size_worker_threads(16, false), (16, 8));
        assert_eq!(size_worker_threads(12, false), (12, 6));
    }

    #[test]
    fn the_core_always_gets_one_thread_per_core() {
        for available in MIN_NODE_THREADS..=256 {
            for is_validator in [true, false] {
                let (node, _) = size_worker_threads(available, is_validator);
                assert_eq!(
                    node, available,
                    "core throttled below the machine size with {available} cores \
                     (is_validator: {is_validator})",
                );
            }
        }
    }

    #[test]
    fn both_runtimes_always_get_their_minimum_threads() {
        for available in 1..=256 {
            for is_validator in [true, false] {
                let (node, serving) = size_worker_threads(available, is_validator);
                assert!(node >= MIN_NODE_THREADS);
                assert!(serving >= MIN_SERVING_THREADS);
            }
        }
    }
}
