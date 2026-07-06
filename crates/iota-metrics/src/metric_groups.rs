// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Predefined groupings of the node's Prometheus metrics.
//!
//! Most groups are filter-based: they key on the module path of the metrics
//! (not the metric name, because module paths are stable and names are not) and
//! are rendered into a `METRICS_FILTER`-style string via
//! [`MetricGroups::to_filter_string`].
//!
//! Each filter-based group is set to a [`MetricLevel`], a verbosity threshold.
//! Individual metrics declare their own level where they are registered
//! (defaulting to [`MetricLevel::Debug`]); metrics used by the fullnode Grafana
//! dashboard are tagged [`MetricLevel::Warn`]. A group registers a metric in
//! its modules when the group's level is at least as verbose as the metric's:
//!
//! - `off` registers nothing;
//! - `warn` (the group default) registers only the `warn`-tagged (dashboard)
//!   metrics;
//! - `info` registers `warn` and `info` metrics;
//! - `debug` registers everything except `trace`-tagged metrics;
//! - `trace` registers everything.
//!
//! Note the two defaults differ: an untagged metric is registered at level
//! `debug`, while a group defaults to the `warn` threshold, so the default
//! config keeps only the dashboard metrics.
//!
//! The `hw` hardware metrics are registered as a `prometheus` `Collector` and
//! so bypass the filtering macros entirely. They cannot be filtered, only
//! registered or not, so the `hardware` group is a plain on/off switch read at
//! registration time and does not appear in
//! [`MetricGroups::to_filter_string`].

pub use prometheus_filtered::MetricLevel;
use serde::{Deserialize, Serialize};

/// Per-group verbosity levels for the node's predefined Prometheus metric
/// groups.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct MetricGroups {
    /// Consensus and block production.
    ///
    /// Modules: `starfish_core`, `iota_core::consensus_adapter`,
    /// `iota_core::consensus_manager`, `iota_core::consensus_validator`,
    /// `iota_core::epoch::consensus_store_pruner`.
    pub consensus: MetricLevel,
    /// Transaction execution and caching.
    ///
    /// Modules: `iota_core::execution_cache`, `iota_core::global_state_hasher`,
    /// `iota_core::module_cache_metrics`.
    pub execution: MetricLevel,
    /// Checkpoint building, certification, and execution.
    ///
    /// Modules: `iota_core::checkpoints`.
    pub checkpoints: MetricLevel,
    /// Transaction submission and finality.
    ///
    /// Modules: `iota_core::quorum_driver`, `iota_core::transaction_driver`,
    /// `iota_core::transaction_orchestrator`,
    /// `iota_core::validator_tx_finalizer`.
    pub transactions: MetricLevel,
    /// Authority request handling and validation.
    ///
    /// Modules: `iota_core::authority` (incl. the authority store and pruner),
    /// `iota_core::safe_client`, `iota_core::signature_verifier`,
    /// `iota_core::validator_client_monitor`.
    ///
    /// The `iota_core::authority` directive is a module-path prefix, so it also
    /// covers the sibling modules `iota_core::authority_aggregator`,
    /// `iota_core::authority_client`, and `iota_core::authority_server`.
    pub authority: MetricLevel,
    /// Spam/abuse traffic control.
    ///
    /// Modules: `iota_core::traffic_controller`.
    pub traffic_control: MetricLevel,
    /// Peer-to-peer networking and state sync.
    ///
    /// Modules: `iota_network::discovery`, `iota_network::randomness`,
    /// `iota_network::state_sync`.
    pub network: MetricLevel,
    /// Persistent storage. The authority object store is part of the
    /// `authority` group, not this one.
    ///
    /// Modules: `typed_store`, `iota_storage`,
    /// `iota_core::db_checkpoint_handler`.
    pub storage: MetricLevel,
    /// API servers and RPC-facing indexes.
    ///
    /// Modules: `iota_json_rpc`, `iota_grpc_server`, `iota_graphql_rpc`,
    /// `iota_core::jsonrpc_index`, `iota_core::subscription_handler`.
    pub rpc: MetricLevel,
    /// Epoch reconfiguration.
    ///
    /// Modules: `iota_core::epoch::epoch_metrics`.
    pub epoch: MetricLevel,
    /// Host hardware metrics (CPU / memory / disk). Registered as a collector,
    /// so it cannot be level-filtered — only switched on or off.
    pub hardware: bool,
}

impl Default for MetricGroups {
    fn default() -> Self {
        Self {
            consensus: MetricLevel::Warn,
            execution: MetricLevel::Warn,
            checkpoints: MetricLevel::Warn,
            transactions: MetricLevel::Warn,
            authority: MetricLevel::Warn,
            traffic_control: MetricLevel::Warn,
            network: MetricLevel::Warn,
            storage: MetricLevel::Warn,
            rpc: MetricLevel::Warn,
            epoch: MetricLevel::Warn,
            hardware: true,
        }
    }
}

impl MetricGroups {
    /// Maps each filter-based group's configured level to the module paths it
    /// covers.
    fn group_modules(&self) -> [(MetricLevel, &'static [&'static str]); 10] {
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

    /// Renders each group's level into a `METRICS_FILTER`-style directive
    /// string. Groups at `trace` are skipped: `trace` covers every metric
    /// level, so their directives would be no-ops.
    pub fn to_filter_string(&self) -> String {
        let mut directives = Vec::new();
        for (level, modules) in self.group_modules() {
            let token = match level {
                MetricLevel::Off => "off",
                MetricLevel::Warn => "warn",
                MetricLevel::Info => "info",
                MetricLevel::Debug => "debug",
                MetricLevel::Trace => continue,
            };
            for module in modules {
                directives.push(format!("{module}={token}"));
            }
        }
        directives.join(",")
    }
}

#[cfg(test)]
mod tests {
    use super::{MetricGroups, MetricLevel};

    fn all_trace() -> MetricGroups {
        MetricGroups {
            consensus: MetricLevel::Trace,
            execution: MetricLevel::Trace,
            checkpoints: MetricLevel::Trace,
            transactions: MetricLevel::Trace,
            authority: MetricLevel::Trace,
            traffic_control: MetricLevel::Trace,
            network: MetricLevel::Trace,
            storage: MetricLevel::Trace,
            rpc: MetricLevel::Trace,
            epoch: MetricLevel::Trace,
            hardware: true,
        }
    }

    #[test]
    fn metric_groups_all_trace_is_noop() {
        // `trace` groups are skipped, so an all-`trace` config registers
        // everything and renders an empty filter.
        assert_eq!(all_trace().to_filter_string(), "");
    }

    #[test]
    fn metric_groups_default_trims_to_dashboard() {
        // The default (all groups `warn`) renders `{module}=warn` for every
        // group's modules, so only the `warn`-tagged (dashboard) metrics
        // survive.
        let filter = MetricGroups::default().to_filter_string();
        assert!(!filter.is_empty());
        assert!(filter.contains("starfish_core=warn"));
        assert!(filter.contains("iota_core::execution_cache=warn"));
        assert!(filter.contains("iota_grpc_server=warn"));
        // No group renders a `debug` directive.
        assert!(!filter.contains("=debug"));
    }

    #[test]
    fn metric_groups_renders_level_per_module() {
        let groups = MetricGroups {
            execution: MetricLevel::Warn,
            checkpoints: MetricLevel::Debug,
            epoch: MetricLevel::Off,
            ..all_trace()
        };
        assert_eq!(
            groups.to_filter_string(),
            "iota_core::execution_cache=warn,iota_core::global_state_hasher=warn,\
             iota_core::module_cache_metrics=warn,iota_core::checkpoints=debug,\
             iota_core::epoch::epoch_metrics=off"
        );
    }

    #[test]
    fn metric_groups_hardware_absent_from_filter() {
        let groups = MetricGroups {
            hardware: false,
            ..all_trace()
        };
        assert_eq!(groups.to_filter_string(), "");
    }

    #[test]
    fn metric_groups_partial_config_defaults_to_warn() {
        // Omitted groups default to `warn`; the explicitly-set group keeps its
        // value.
        let groups: MetricGroups = serde_yaml::from_str("traffic-control: off").unwrap();
        assert_eq!(groups.consensus, MetricLevel::Warn);
        assert_eq!(groups.traffic_control, MetricLevel::Off);
        assert!(groups.hardware);
        let filter = groups.to_filter_string();
        assert!(filter.contains("iota_core::traffic_controller=off"));
        assert!(filter.contains("starfish_core=warn"));
    }
}
