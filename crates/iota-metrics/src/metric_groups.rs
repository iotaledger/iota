// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Predefined groupings of the node's Prometheus metrics.
//!
//! Most groups are filter-based: they key on the module path of the metrics
//! (preferred over the metric name, because module paths are stable and names
//! are not) and are rendered into a `METRICS_FILTER`-style string via
//! [`MetricGroups::to_filter_string`].
//!
//! Each filter-based group is set to a [`MetricLevel`], a verbosity threshold.
//! Individual metrics declare their own level where they are registered,
//! defaulting to [`MetricLevel::Debug`]. A group's level decides which of its
//! metrics are **exposed** on the metrics endpoint — a metric is exposed when
//! the group's level is at least as verbose as the metric's:
//!
//! - `off` exposes nothing;
//! - `warn` (the group default) exposes only `warn`-tagged metrics;
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
//! config exposes only the `warn`-tagged metrics.
//!
//! The node applies [`MetricGroups::default()`] when the config omits
//! `metrics.groups` entirely, so an omitted and an empty section behave the
//! same.
//!
//! The `hw` hardware metrics are one collector rather than macro-registered
//! metrics, so the whole group shares a single level: the `hardware` group's
//! level exposes the collector (`off` hides it, any other level exposes it),
//! and — like the other groups — it can be changed at runtime.

use std::collections::BTreeMap;

pub use prometheus_filtered::MetricLevel;
use serde::{Deserialize, Serialize};

/// The `METRICS_FILTER` token for a level.
fn level_string(level: MetricLevel) -> &'static str {
    match level {
        MetricLevel::Off => "off",
        MetricLevel::Warn => "warn",
        MetricLevel::Info => "info",
        MetricLevel::Debug => "debug",
        MetricLevel::Trace => "trace",
    }
}

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
    /// Transaction execution and caching, including the Move bytecode verifier
    /// and execution-limit meters.
    ///
    /// Modules: `iota_core::execution_cache`, `iota_core::global_state_hasher`,
    /// `iota_core::module_cache_metrics`, `iota_types::metrics`.
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
    /// `iota_core::validator_client_monitor`. Also the `authority_grpc_*`
    /// transport metrics of the validator gRPC server, matched by name prefix
    /// because their module (`iota_node::metrics`) is shared with the `epoch`
    /// group's protocol-version gauges.
    ///
    /// The `iota_core::authority` directive is a module-path prefix, so it also
    /// covers the sibling modules `iota_core::authority_aggregator`,
    /// `iota_core::authority_client`, and `iota_core::authority_server`.
    pub authority: MetricLevel,
    /// Spam/abuse traffic control, including the transaction-deny config
    /// gauges.
    ///
    /// Modules: `iota_core::traffic_controller`,
    /// `iota_config::node_config_metrics`.
    pub traffic_control: MetricLevel,
    /// Peer-to-peer networking and state sync.
    ///
    /// Modules: `iota_network::discovery`, `iota_network::randomness`,
    /// `iota_network::state_sync`.
    pub network: MetricLevel,
    /// The anemo P2P transport underneath the `network` group's subsystems.
    /// Kept separate from `network` because most of these are per-peer
    /// (connection/RTT/packet-loss gauges) and therefore high-cardinality, so
    /// they can be silenced independently.
    ///
    /// Modules: `iota_metrics::metrics_network`.
    pub p2p: MetricLevel,
    /// Persistent storage, including archive writes/reads and state snapshot
    /// uploads. The authority object store is part of the `authority` group,
    /// not this one.
    ///
    /// Modules: `typed_store`, `iota_storage`,
    /// `iota_core::db_checkpoint_handler`, `iota_snapshot`.
    pub storage: MetricLevel,
    /// API servers and RPC-facing indexes.
    ///
    /// Modules: `iota_json_rpc`, `iota_grpc_server`, `iota_graphql_rpc`,
    /// `iota_core::jsonrpc_index`, `iota_core::subscription_handler`.
    pub rpc: MetricLevel,
    /// Epoch reconfiguration and protocol versioning.
    ///
    /// Modules: `iota_core::epoch::epoch_metrics`. Also the
    /// `iota_current/binary/configured_max_protocol_version` gauges, matched by
    /// name because their module (`iota_node::metrics`) is shared with the
    /// `authority` group's gRPC transport metrics.
    pub epoch: MetricLevel,
    /// Async-runtime and process health: monitored tokio tasks, channels, and
    /// scopes, per-runtime tokio scheduler metrics (`tokio_runtime_*`), thread
    /// stalls, invariant violations, and tracing span latencies.
    ///
    /// Modules: `iota_metrics` (except the `hardware` and `p2p` group
    /// submodules), `telemetry_subscribers`.
    pub runtime: MetricLevel,
    /// Host hardware metrics (CPU / memory / disk). One collector, so the level
    /// applies to the whole group rather than individual metrics: `off` hides
    /// it and every other level exposes it.
    ///
    /// Rendered as an `iota_metrics::hardware_metrics` directive, the module
    /// where the collector is registered.
    pub hardware: MetricLevel,
    /// Free-form overrides for module paths or metric names not covered by the
    /// named groups.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub overrides: BTreeMap<String, MetricLevel>,
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
            p2p: MetricLevel::Warn,
            storage: MetricLevel::Warn,
            rpc: MetricLevel::Warn,
            epoch: MetricLevel::Warn,
            runtime: MetricLevel::Warn,
            hardware: MetricLevel::Warn,
            overrides: BTreeMap::new(),
        }
    }
}

impl MetricGroups {
    /// Returns the filter patterns a group covers — keyed
    /// by the group's config key. `None` for unknown groups.
    pub fn modules_for_group(group: &str) -> Option<&'static [&'static str]> {
        Some(match group {
            "runtime" => &["iota_metrics", "telemetry_subscribers"],
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
                "iota_types::metrics",
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
                // Name prefix: the validator gRPC transport metrics live in
                // `iota_node::metrics` together with the `epoch` group's
                // protocol-version gauges.
                "authority_grpc",
            ],
            "traffic-control" => &[
                "iota_core::traffic_controller",
                "iota_config::node_config_metrics",
            ],
            "network" => &[
                "iota_network::discovery",
                "iota_network::randomness",
                "iota_network::state_sync",
            ],
            "p2p" => &["iota_metrics::metrics_network"],
            "storage" => &[
                "typed_store",
                "iota_storage",
                "iota_core::db_checkpoint_handler",
                "iota_snapshot",
            ],
            "rpc" => &[
                "iota_json_rpc",
                "iota_grpc_server",
                "iota_graphql_rpc",
                "iota_core::jsonrpc_index",
                "iota_core::subscription_handler",
            ],
            "epoch" => &[
                "iota_core::epoch::epoch_metrics",
                // Name prefixes: these gauges live in `iota_node::metrics`.
                "iota_current_protocol_version",
                "iota_binary_max_protocol_version",
                "iota_configured_max_protocol_version",
            ],
            "hardware" => &["iota_metrics::hardware_metrics"],
            _ => return None,
        })
    }

    /// The predefined groups paired with their configured levels.
    fn group_levels(&self) -> [(&'static str, MetricLevel); 13] {
        [
            ("runtime", self.runtime),
            ("consensus", self.consensus),
            ("execution", self.execution),
            ("checkpoints", self.checkpoints),
            ("transactions", self.transactions),
            ("authority", self.authority),
            ("traffic-control", self.traffic_control),
            ("network", self.network),
            ("p2p", self.p2p),
            ("storage", self.storage),
            ("rpc", self.rpc),
            ("epoch", self.epoch),
            ("hardware", self.hardware),
        ]
    }

    /// Maps each group's configured level to the filter patterns it covers.
    ///
    /// Directive order does not matter: where one group's module prefix also
    /// matches another group's submodules (e.g. `runtime`'s `iota_metrics`
    /// covers the `p2p` and `hardware` groups'), the more specific pattern
    /// wins.
    fn group_modules(&self) -> [(MetricLevel, &'static [&'static str]); 13] {
        self.group_levels().map(|(group, level)| {
            (
                level,
                Self::modules_for_group(group).expect("group has modules"),
            )
        })
    }

    /// Renders the levels into a `METRICS_FILTER`-style directive string: the
    /// `default` threshold as the catch-all directive, then one directive per
    /// group module (including the hardware group's). The group levels
    /// override the catch-all for their modules, being more specific.
    pub fn to_filter_string(&self) -> String {
        let mut directives = vec![level_string(self.default).to_owned()];
        for (level, modules) in self.group_modules() {
            for module in modules {
                directives.push(format!("{module}={}", level_string(level)));
            }
        }
        directives.extend(self.override_directives());
        directives.join(",")
    }

    /// Renders the levels into a group-form directive string.
    /// Keyed by group name rather than expanded
    /// to module paths, so the admin endpoint can echo the config compactly.
    pub fn to_display_string(&self) -> String {
        let mut directives = vec![format!("default={}", level_string(self.default))];
        for (group, level) in self.group_levels() {
            directives.push(format!("{group}={}", level_string(level)));
        }
        directives.extend(self.override_directives());
        directives.join(",")
    }

    /// Expands group names in a `pattern=LEVEL` directive string into the
    /// groups' filter patterns; other directives pass through unchanged.
    /// Any invalid directive rejects the whole string, with every offending
    /// directive reported.
    pub fn expand_directives(filter: &str) -> Result<String, String> {
        let (directives, errors) = Self::expand_startup_directives(filter);
        if errors.is_empty() {
            Ok(directives)
        } else {
            Err(errors.join("; "))
        }
    }

    /// Like [`Self::expand_directives`], but for startup use: an invalid
    /// directive is dropped instead of rejecting the whole string, its error
    /// message returned alongside the expanded directives.
    pub fn expand_startup_directives(filter: &str) -> (String, Vec<String>) {
        let mut directives = Vec::new();
        let mut errors = Vec::new();
        for part in prometheus_filtered::directive_parts(filter) {
            match Self::expand_directive(part) {
                Ok(expanded) => directives.extend(expanded),
                Err(err) => errors.push(err),
            }
        }
        (directives.join(","), errors)
    }

    fn expand_directive(part: &str) -> Result<Vec<String>, String> {
        prometheus_filtered::validate_directive(part)?;
        let (pattern, level) = prometheus_filtered::split_directive(part);
        if pattern == "default" {
            return Ok(vec![level.to_owned()]);
        }
        Ok(match Self::modules_for_group(pattern) {
            Some(modules) => modules
                .iter()
                .map(|module| format!("{module}={level}"))
                .collect(),
            None => vec![part.to_owned()],
        })
    }

    /// Renders the free-form overrides as `pattern=LEVEL` directives.
    fn override_directives(&self) -> impl Iterator<Item = String> + '_ {
        self.overrides
            .iter()
            .map(|(pattern, level)| format!("{pattern}={}", level_string(*level)))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

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
            p2p: MetricLevel::Trace,
            storage: MetricLevel::Trace,
            rpc: MetricLevel::Trace,
            epoch: MetricLevel::Trace,
            runtime: MetricLevel::Trace,
            hardware: MetricLevel::Trace,
            overrides: BTreeMap::new(),
        }
    }

    #[test]
    fn metric_groups_default_trims_to_warn_tagged() {
        // The default (all groups `warn`) renders `{module}=warn` for every
        // group's modules, so only the `warn`-tagged metrics are exposed;
        // non-grouped modules fall to the `info` catch-all.
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
    fn to_display_string_keys_by_group_name() {
        // The display form keeps group names rather than expanding to modules.
        let display = MetricGroups {
            consensus: MetricLevel::Off,
            storage: MetricLevel::Trace,
            ..MetricGroups::default()
        }
        .to_display_string();
        assert!(display.starts_with("default=info,"));
        assert!(display.contains("consensus=off"));
        assert!(display.contains("storage=trace"));
        assert!(display.contains("hardware=warn"));
        // No module paths leak into the display form.
        assert!(!display.contains("::"));
        assert!(!display.contains("starfish_core"));
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
        // Ungrouped modules follow the `default` catch-all (`trace` here).
        assert!(filter.is_exposed("x", "iota_node::some_module", MetricLevel::Trace));
    }

    #[test]
    fn metric_groups_runtime_prefix_is_overridden_by_submodule_groups() {
        // `runtime` covers the whole `iota_metrics` crate by module prefix,
        // but the `p2p` and `hardware` submodules belong to their own
        // groups, whose more specific patterns win.
        let groups = MetricGroups {
            runtime: MetricLevel::Trace,
            p2p: MetricLevel::Warn,
            hardware: MetricLevel::Off,
            ..all_trace()
        };
        let filter = Filter::parse(&groups.to_filter_string());
        assert!(filter.is_exposed("monitored_tasks", "iota_metrics", MetricLevel::Trace));
        assert!(filter.is_exposed(
            "network_peer_rtt",
            "iota_metrics::metrics_network",
            MetricLevel::Warn
        ));
        assert!(!filter.is_exposed(
            "network_peer_rtt",
            "iota_metrics::metrics_network",
            MetricLevel::Info
        ));
        assert!(!filter.is_exposed(
            "hw_cpu_core_count",
            "iota_metrics::hardware_metrics",
            MetricLevel::Warn
        ));
    }

    #[test]
    fn metric_groups_name_patterns_split_shared_module() {
        // The protocol-version gauges and the gRPC transport metrics share the
        // `iota_node::metrics` module but belong to different groups, matched
        // by metric-name prefix.
        let groups = MetricGroups {
            epoch: MetricLevel::Off,
            authority: MetricLevel::Debug,
            ..all_trace()
        };
        let filter = Filter::parse(&groups.to_filter_string());
        assert!(!filter.is_exposed(
            "iota_current_protocol_version",
            "iota_node::metrics",
            MetricLevel::Warn
        ));
        assert!(filter.is_exposed(
            "authority_grpc_requests",
            "iota_node::metrics",
            MetricLevel::Debug
        ));
    }

    #[test]
    fn modules_for_group_covers_every_group() {
        // Every group resolves to a non-empty module list; the rendered
        // filter contains exactly those modules.
        let filter = MetricGroups::default().to_filter_string();
        for group in [
            "consensus",
            "execution",
            "checkpoints",
            "transactions",
            "authority",
            "traffic-control",
            "network",
            "p2p",
            "storage",
            "rpc",
            "epoch",
            "runtime",
            "hardware",
        ] {
            let modules = MetricGroups::modules_for_group(group)
                .unwrap_or_else(|| panic!("group {group} has no modules"));
            assert!(!modules.is_empty());
            for module in modules {
                assert!(filter.contains(&format!("{module}=warn")));
            }
        }
        // Unknown names resolve to nothing.
        assert_eq!(MetricGroups::modules_for_group("bogus"), None);
    }

    #[test]
    fn expand_directives_expands_groups_and_passes_raw_patterns() {
        assert_eq!(
            MetricGroups::expand_directives("checkpoints=off,epoch=debug").unwrap(),
            "iota_core::checkpoints=off,iota_core::epoch::epoch_metrics=debug,\
             iota_current_protocol_version=debug,iota_binary_max_protocol_version=debug,\
             iota_configured_max_protocol_version=debug"
        );
        // Raw module paths, metric-name prefixes, and bare global levels are
        // kept verbatim; level validity is checked up front.
        assert_eq!(
            MetricGroups::expand_directives("typed_store=warn, uptime=off ,trace").unwrap(),
            "typed_store=warn,uptime=off,trace"
        );
        assert_eq!(MetricGroups::expand_directives("").unwrap(), "");
        // The `default` group becomes the bare catch-all level.
        assert_eq!(
            MetricGroups::expand_directives("default=info,traffic-control=off").unwrap(),
            "info,iota_core::traffic_controller=off,iota_config::node_config_metrics=off"
        );
        // The single-collector hardware group expands like any other.
        assert_eq!(
            MetricGroups::expand_directives("hardware=off").unwrap(),
            "iota_metrics::hardware_metrics=off"
        );
    }

    #[test]
    fn expand_startup_directives_drops_bad_directives() {
        // An invalid directive is dropped and reported; the rest still expand.
        let (expanded, errors) =
            MetricGroups::expand_startup_directives("consensus=bogus,traffic-control=off");
        assert_eq!(
            expanded,
            "iota_core::traffic_controller=off,iota_config::node_config_metrics=off"
        );
        assert_eq!(errors.len(), 1);
        assert!(
            errors[0].contains("consensus=bogus"),
            "unexpected error: {}",
            errors[0]
        );
    }

    #[test]
    fn expand_directives_rejects_invalid_input() {
        // An invalid level fails the whole string, citing every offending
        // directive as the caller wrote it — not its expansion.
        let err =
            MetricGroups::expand_directives("consensus=bogus,storage=warn,epoch=nah").unwrap_err();
        assert!(err.contains("consensus=bogus"), "unexpected error: {err}");
        assert!(err.contains("epoch=nah"), "unexpected error: {err}");
    }

    #[test]
    fn expand_directives_is_order_independent() {
        // A `default` level after a group directive does not cancel it.
        for input in [
            "traffic-control=off,default=info",
            "default=info,traffic-control=off",
        ] {
            let expanded = MetricGroups::expand_directives(input).unwrap();
            let filter = Filter::parse(&expanded);
            assert!(
                !filter.is_exposed("x", "iota_core::traffic_controller", MetricLevel::Warn),
                "{input} -> {expanded}"
            );
            assert!(
                filter.is_exposed("x", "iota_core::authority", MetricLevel::Info),
                "{input} -> {expanded}"
            );
            assert!(
                !filter.is_exposed("x", "iota_core::authority", MetricLevel::Debug),
                "{input} -> {expanded}"
            );
        }
        // With three nested module prefixes, each level applies to its own
        // subtree: the most specific matching directive wins.
        let expanded = MetricGroups::expand_directives(
            "iota_metrics::metrics_network::inbound=off,transport=debug,runtime=trace",
        )
        .unwrap();
        let filter = Filter::parse(&expanded);
        assert!(
            !filter.is_exposed(
                "x",
                "iota_metrics::metrics_network::inbound",
                MetricLevel::Warn
            ),
            "{expanded}"
        );
        assert!(
            filter.is_exposed("x", "iota_metrics::metrics_network", MetricLevel::Debug),
            "{expanded}"
        );
        assert!(
            filter.is_exposed("x", "iota_metrics", MetricLevel::Trace),
            "{expanded}"
        );
        // A later directive for the same pattern still wins (last match):
        // here `runtime` expands to the raw directive's `iota_metrics`
        // pattern.
        let expanded = MetricGroups::expand_directives("iota_metrics=off,runtime=warn").unwrap();
        let filter = Filter::parse(&expanded);
        assert!(filter.is_exposed("x", "iota_metrics", MetricLevel::Warn));
    }

    #[test]
    fn metric_groups_config_parsing() {
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

        // A typo'd group name must fail config load instead of silently
        // leaving the intended group at its default.
        assert!(serde_yaml::from_str::<MetricGroups>("traffic_control: off").is_err());
        assert!(serde_yaml::from_str::<MetricGroups>("bogus: warn").is_err());
    }

    #[test]
    fn metric_groups_config_allows_free_overrides() {
        // Module- and metric-level directives that no named group covers go in
        // the `overrides` map, keeping group-name typo protection intact.
        let groups: MetricGroups = serde_yaml::from_str(
            "consensus: off\n\
             overrides:\n  \"iota_core::authority::foo\": trace\n  bespoke_metric: off",
        )
        .unwrap();
        assert_eq!(groups.consensus, MetricLevel::Off);

        let filter_string = groups.to_filter_string();
        assert!(filter_string.contains("iota_core::authority::foo=trace"));
        assert!(filter_string.contains("bespoke_metric=off"));

        let filter = Filter::parse(&filter_string);
        // The longer override pattern wins over the `authority` group directive.
        assert!(filter.is_exposed("x", "iota_core::authority::foo", MetricLevel::Trace));
        assert!(!filter.is_exposed("bespoke_metric_total", "somewhere", MetricLevel::Warn));
    }
}
