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
//! (defaulting to [`MetricLevel::Debug`]); metrics used by the shipped Grafana
//! dashboards are tagged [`MetricLevel::Warn`]. A group's level decides which
//! of its metrics are **exposed** on the metrics endpoint — a metric is
//! exposed when the group's level is at least as verbose as the metric's:
//!
//! - `off` exposes nothing;
//! - `warn` (the group default) exposes only the `warn`-tagged (dashboard)
//!   metrics;
//! - `info` exposes `warn` and `info` metrics;
//! - `debug` exposes everything except `trace`-tagged metrics;
//! - `trace` exposes everything.
//!
//! Metrics whose module belongs to no group are covered by the `default`
//! threshold (`info` unless configured), rendered as the leading catch-all
//! directive.
//!
//! The levels never affect collection: a filter-based group's metrics are
//! registered and keep collecting regardless of the configured level.
//!
//! Note the two defaults differ: an untagged metric is exposed from level
//! `debug`, while a group defaults to the `warn` threshold, so the default
//! config exposes only the dashboard metrics.
//!
//! The node applies [`MetricGroups::default()`] when the config omits
//! `metrics.groups` entirely, so an omitted and an empty section behave the
//! same.
//!
//! The `hw` hardware metrics are registered as a prometheus collector and
//! so bypass the filtering macros entirely. They cannot be level-filtered
//! individually: the `hardware` group's level is read once at registration
//! time — [`MetricLevel::Off`] skips the whole group, any other level
//! registers it.

pub use prometheus_filtered::MetricLevel;
use serde::{Deserialize, Serialize};

/// Per-group verbosity levels for the node's predefined Prometheus metric
/// groups.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", default, deny_unknown_fields)]
pub struct MetricGroups {
    /// Exposure threshold for metrics whose module belongs to no group.
    pub default: MetricLevel,
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
    /// so individual metrics cannot be level-filtered: `off` skips the whole
    /// group and every other level registers it.
    pub hardware: MetricLevel,
}

impl Default for MetricGroups {
    fn default() -> Self {
        Self {
            default: MetricLevel::Info,
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
            hardware: MetricLevel::Warn,
        }
    }
}

impl MetricGroups {
    /// Returns the module paths a filter-based group covers, keyed by the
    /// group's config. `None` for unknown groups and for `hardware`, which is
    /// not filter-based.
    pub fn modules_for_group(group: &str) -> Option<&'static [&'static str]> {
        Some(match group {
            "consensus" => &[
                "starfish_core",
                "iota_core::consensus_adapter",
                "iota_core::consensus_manager",
                "iota_core::consensus_validator",
                "iota_core::epoch::consensus_store_pruner",
            ],
            "execution" => &[
                "iota_core::execution_cache",
                "iota_core::global_state_hasher",
                "iota_core::module_cache_metrics",
            ],
            "checkpoints" => &["iota_core::checkpoints"],
            "transactions" => &[
                "iota_core::quorum_driver",
                "iota_core::transaction_driver",
                "iota_core::transaction_orchestrator",
                "iota_core::validator_tx_finalizer",
            ],
            "authority" => &[
                "iota_core::authority",
                "iota_core::safe_client",
                "iota_core::signature_verifier",
                "iota_core::validator_client_monitor",
            ],
            "traffic-control" => &["iota_core::traffic_controller"],
            "network" => &[
                "iota_network::discovery",
                "iota_network::randomness",
                "iota_network::state_sync",
            ],
            "storage" => &[
                "typed_store",
                "iota_storage",
                "iota_core::db_checkpoint_handler",
            ],
            "rpc" => &[
                "iota_json_rpc",
                "iota_grpc_server",
                "iota_graphql_rpc",
                "iota_core::jsonrpc_index",
                "iota_core::subscription_handler",
            ],
            "epoch" => &["iota_core::epoch::epoch_metrics"],
            _ => return None,
        })
    }

    /// Maps each filter-based group's configured level to the module paths it
    /// covers.
    fn group_modules(&self) -> [(MetricLevel, &'static [&'static str]); 10] {
        [
            ("consensus", self.consensus),
            ("execution", self.execution),
            ("checkpoints", self.checkpoints),
            ("transactions", self.transactions),
            ("authority", self.authority),
            ("traffic-control", self.traffic_control),
            ("network", self.network),
            ("storage", self.storage),
            ("rpc", self.rpc),
            ("epoch", self.epoch),
        ]
        .map(|(group, level)| {
            (
                level,
                Self::modules_for_group(group).expect("filter-based group has modules"),
            )
        })
    }

    /// Renders the levels into a `METRICS_FILTER`-style directive string: the
    /// `default` threshold as the leading catch-all directive, then one
    /// directive per group module. Later directives win, so the group levels
    /// override the catch-all for their modules.
    pub fn to_filter_string(&self) -> String {
        fn token(level: MetricLevel) -> &'static str {
            match level {
                MetricLevel::Off => "off",
                MetricLevel::Warn => "warn",
                MetricLevel::Info => "info",
                MetricLevel::Debug => "debug",
                MetricLevel::Trace => "trace",
            }
        }
        let mut directives = vec![token(self.default).to_owned()];
        for (level, modules) in self.group_modules() {
            for module in modules {
                directives.push(format!("{module}={}", token(level)));
            }
        }
        directives.join(",")
    }
}

#[cfg(test)]
mod tests {
    use prometheus_filtered::Filter;

    use super::{MetricGroups, MetricLevel};

    fn all_trace() -> MetricGroups {
        MetricGroups {
            default: MetricLevel::Trace,
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
            hardware: MetricLevel::Trace,
        }
    }

    #[test]
    fn metric_groups_all_trace_exposes_everything() {
        // An all-`trace` config exposes every metric, grouped or not.
        let filter = Filter::parse(&all_trace().to_filter_string());
        assert!(filter.is_exposed("x", "starfish_core::metrics", MetricLevel::Trace));
        assert!(filter.is_exposed("x", "iota_node::some_module", MetricLevel::Trace));
    }

    #[test]
    fn metric_groups_default_trims_to_dashboard() {
        // The default (all groups `warn`) renders `{module}=warn` for every
        // group's modules, so only the `warn`-tagged (dashboard) metrics
        // are exposed; non-grouped modules fall to the `info` catch-all.
        let filter_string = MetricGroups::default().to_filter_string();
        assert!(filter_string.starts_with("info,"));
        assert!(filter_string.contains("starfish_core=warn"));
        assert!(filter_string.contains("iota_core::execution_cache=warn"));
        assert!(filter_string.contains("iota_grpc_server=warn"));
        // No group renders a `debug` directive.
        assert!(!filter_string.contains("=debug"));

        let filter = Filter::parse(&filter_string);
        assert!(filter.is_exposed("x", "starfish_core::metrics", MetricLevel::Warn));
        assert!(!filter.is_exposed("x", "starfish_core::metrics", MetricLevel::Info));
        assert!(filter.is_exposed("x", "iota_node::some_module", MetricLevel::Info));
        assert!(!filter.is_exposed("x", "iota_node::some_module", MetricLevel::Debug));
    }

    #[test]
    fn metric_groups_renders_level_per_module() {
        let groups = MetricGroups {
            execution: MetricLevel::Warn,
            checkpoints: MetricLevel::Debug,
            epoch: MetricLevel::Off,
            ..all_trace()
        };
        let filter_string = groups.to_filter_string();
        assert!(filter_string.starts_with("trace,"));
        assert!(filter_string.contains("iota_core::execution_cache=warn"));
        assert!(filter_string.contains("iota_core::checkpoints=debug"));
        assert!(filter_string.contains("iota_core::epoch::epoch_metrics=off"));

        let filter = Filter::parse(&filter_string);
        assert!(filter.is_exposed("x", "iota_core::execution_cache", MetricLevel::Warn));
        assert!(!filter.is_exposed("x", "iota_core::execution_cache", MetricLevel::Info));
        assert!(filter.is_exposed("x", "iota_core::checkpoints", MetricLevel::Debug));
        assert!(!filter.is_exposed("x", "iota_core::checkpoints", MetricLevel::Trace));
        assert!(!filter.is_exposed("x", "iota_core::epoch::epoch_metrics", MetricLevel::Warn));
        // `trace` groups expose everything — their directives are rendered,
        // not skipped, so they are not clipped by the catch-all.
        assert!(filter.is_exposed("x", "iota_core::quorum_driver", MetricLevel::Trace));
    }

    #[test]
    fn metric_groups_hardware_absent_from_filter() {
        let groups = MetricGroups {
            hardware: MetricLevel::Off,
            ..all_trace()
        };
        // `hardware` is gated at registration, not via the filter.
        assert!(!groups.to_filter_string().contains("hardware"));
    }

    #[test]
    fn modules_for_group_covers_filter_based_groups_only() {
        // Every filter-based group resolves to a non-empty module list; the
        // rendered filter contains exactly those modules.
        let filter = MetricGroups::default().to_filter_string();
        for group in [
            "consensus",
            "execution",
            "checkpoints",
            "transactions",
            "authority",
            "traffic-control",
            "network",
            "storage",
            "rpc",
            "epoch",
        ] {
            let modules = MetricGroups::modules_for_group(group)
                .unwrap_or_else(|| panic!("group {group} has no modules"));
            assert!(!modules.is_empty());
            for module in modules {
                assert!(filter.contains(&format!("{module}=warn")));
            }
        }
        // `hardware` is not filter-based; unknown names resolve to nothing.
        assert_eq!(MetricGroups::modules_for_group("hardware"), None);
        assert_eq!(MetricGroups::modules_for_group("bogus"), None);
    }

    #[test]
    fn metric_groups_partial_config_defaults_to_warn() {
        // Omitted groups default to `warn`; the explicitly-set group keeps its
        // value.
        let groups: MetricGroups = serde_yaml::from_str("traffic-control: off").unwrap();
        assert_eq!(groups.default, MetricLevel::Info);
        assert_eq!(groups.consensus, MetricLevel::Warn);
        assert_eq!(groups.traffic_control, MetricLevel::Off);
        assert_eq!(groups.hardware, MetricLevel::Warn);
        let filter = groups.to_filter_string();
        assert!(filter.contains("iota_core::traffic_controller=off"));
        assert!(filter.contains("starfish_core=warn"));
    }

    #[test]
    fn metric_groups_rejects_unknown_group_names() {
        // A typo'd group name must fail config load instead of silently
        // leaving the intended group at its default.
        assert!(serde_yaml::from_str::<MetricGroups>("traffic_control: off").is_err());
        assert!(serde_yaml::from_str::<MetricGroups>("bogus: warn").is_err());
    }
}
