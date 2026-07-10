// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Predefined groupings of the node's Prometheus metrics and the mapping from
//! each group to the modules that register its metrics.
//!
//! Grouping keys are based on the module path of the metrics, not by the name
//! of metrics. Because module paths are stable, and metric names are not.
//!
//! Metrics registered as a `prometheus` `Collector` (e.g. the `hw` hardware
//! metrics) bypass the filtering macros entirely, so they are unaffected by
//! these groups.

use serde::{Deserialize, Serialize};

/// On/off switches for the node's predefined Prometheus metric groups.
/// The default is to enable all groups.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct MetricGroups {
    /// Consensus and block production.
    ///
    /// Modules: `starfish_core`, `iota_core::consensus_adapter`,
    /// `iota_core::consensus_manager`, `iota_core::consensus_validator`,
    /// `iota_core::epoch::consensus_store_pruner`.
    pub consensus: bool,
    /// Transaction execution and caching.
    ///
    /// Modules: `iota_core::execution_cache`, `iota_core::global_state_hasher`,
    /// `iota_core::module_cache_metrics`.
    pub execution: bool,
    /// Checkpoint building, certification, and execution.
    ///
    /// Modules: `iota_core::checkpoints`.
    pub checkpoints: bool,
    /// Transaction submission and finality.
    ///
    /// Modules: `iota_core::quorum_driver`, `iota_core::transaction_driver`,
    /// `iota_core::transaction_orchestrator`,
    /// `iota_core::validator_tx_finalizer`.
    pub transactions: bool,
    /// Authority request handling and validation.
    ///
    /// Modules: `iota_core::authority` (incl. the authority store and pruner),
    /// `iota_core::safe_client`, `iota_core::signature_verifier`,
    /// `iota_core::validator_client_monitor`.
    ///
    /// The `iota_core::authority` directive is a module-path prefix, so it also
    /// covers the sibling modules `iota_core::authority_aggregator`,
    /// `iota_core::authority_client`, and `iota_core::authority_server`.
    pub authority: bool,
    /// Spam/abuse traffic control.
    ///
    /// Modules: `iota_core::traffic_controller`.
    pub traffic_control: bool,
    /// Peer-to-peer networking and state sync.
    ///
    /// Modules: `iota_network::discovery`, `iota_network::randomness`,
    /// `iota_network::state_sync`.
    pub network: bool,
    /// Persistent storage. The authority object store is part of the
    /// `authority` group, not this one.
    ///
    /// Modules: `typed_store`, `iota_storage`,
    /// `iota_core::db_checkpoint_handler`.
    pub storage: bool,
    /// API servers and RPC-facing indexes.
    ///
    /// Modules: `iota_json_rpc`, `iota_grpc_server`, `iota_graphql_rpc`,
    /// `iota_core::jsonrpc_index`, `iota_core::subscription_handler`.
    pub rpc: bool,
    /// Epoch reconfiguration.
    ///
    /// Modules: `iota_core::epoch::epoch_metrics`.
    pub epoch: bool,
}

impl Default for MetricGroups {
    fn default() -> Self {
        // All groups enabled, so an omitted or partial config never silently
        // drops metrics.
        Self {
            consensus: true,
            execution: true,
            checkpoints: true,
            transactions: true,
            authority: true,
            traffic_control: true,
            network: true,
            storage: true,
            rpc: true,
            epoch: true,
        }
    }
}

impl MetricGroups {
    /// Maps each group's enabled flag to the module paths whose metrics it
    /// covers.
    fn group_modules(&self) -> [(bool, &'static [&'static str]); 10] {
        [
            (
                self.consensus,
                &[
                    "starfish_core",
                    "iota_core::consensus_adapter",
                    "iota_core::consensus_manager",
                    "iota_core::consensus_validator",
                    "iota_core::epoch::consensus_store_pruner",
                ],
            ),
            (
                self.execution,
                &[
                    "iota_core::execution_cache",
                    "iota_core::global_state_hasher",
                    "iota_core::module_cache_metrics",
                ],
            ),
            (self.checkpoints, &["iota_core::checkpoints"]),
            (
                self.transactions,
                &[
                    "iota_core::quorum_driver",
                    "iota_core::transaction_driver",
                    "iota_core::transaction_orchestrator",
                    "iota_core::validator_tx_finalizer",
                ],
            ),
            (
                self.authority,
                &[
                    "iota_core::authority",
                    "iota_core::safe_client",
                    "iota_core::signature_verifier",
                    "iota_core::validator_client_monitor",
                ],
            ),
            (self.traffic_control, &["iota_core::traffic_controller"]),
            (
                self.network,
                &[
                    "iota_network::discovery",
                    "iota_network::randomness",
                    "iota_network::state_sync",
                ],
            ),
            (
                self.storage,
                &[
                    "typed_store",
                    "iota_storage",
                    "iota_core::db_checkpoint_handler",
                ],
            ),
            (
                self.rpc,
                &[
                    "iota_json_rpc",
                    "iota_grpc_server",
                    "iota_graphql_rpc",
                    "iota_core::jsonrpc_index",
                    "iota_core::subscription_handler",
                ],
            ),
            (self.epoch, &["iota_core::epoch::epoch_metrics"]),
        ]
    }

    /// Renders the disabled groups into a `METRICS_FILTER`-style directive
    /// string.
    pub fn to_filter_string(&self) -> String {
        let mut directives = Vec::new();
        for (enabled, modules) in self.group_modules() {
            if !enabled {
                for module in modules {
                    directives.push(format!("{module}=off"));
                }
            }
        }
        directives.join(",")
    }
}

#[cfg(test)]
mod tests {
    use super::MetricGroups;

    #[test]
    fn metric_groups_to_filter_string() {
        // All groups enabled by default produces an empty (no-op) filter.
        assert_eq!(MetricGroups::default().to_filter_string(), "");

        // Disabling a group emits `=off` for each of its modules; enabled
        // groups emit nothing.
        let groups = MetricGroups {
            storage: false,
            epoch: false,
            ..Default::default()
        };
        assert_eq!(
            groups.to_filter_string(),
            "typed_store=off,iota_storage=off,iota_core::db_checkpoint_handler=off,\
             iota_core::epoch::epoch_metrics=off"
        );
    }

    #[test]
    fn metric_groups_partial_config_defaults_to_enabled() {
        // Omitted groups must default to enabled so a partial config never
        // silently drops metrics.
        let groups: MetricGroups = serde_yaml::from_str("traffic-control: false").unwrap();
        assert!(groups.consensus);
        assert!(groups.execution);
        assert!(!groups.traffic_control);
        assert_eq!(
            groups.to_filter_string(),
            "iota_core::traffic_controller=off"
        );
    }
}
