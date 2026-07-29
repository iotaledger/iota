// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Generic key-value overrides applied to generated [`NodeConfig`]s.

use std::{fmt, str::FromStr};

use anyhow::{Context, anyhow, bail};
use iota_config::NodeConfig;
use serde_yaml::{Mapping, Value};

/// Node config fields that must not be overridden because they carry node
/// identity or data that has to stay consistent across the network.
const PROTECTED_FIELDS: &[&str] = &[
    "authority-key-pair",
    "protocol-key-pair",
    "account-key-pair",
    "network-key-pair",
    "genesis",
    "migration-tx-data-path",
];

/// Which nodes a [`NodeConfigOverride`] applies to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverrideScope {
    /// Every node in the network.
    All,
    /// All fullnodes.
    Fullnode,
    /// The validator at the given index in the network config.
    Validator(usize),
}

impl fmt::Display for OverrideScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OverrideScope::All => write!(f, "all"),
            OverrideScope::Fullnode => write!(f, "fullnode"),
            OverrideScope::Validator(index) => write!(f, "validator-{index}"),
        }
    }
}

/// A single `[scope:]<path>=<value>` override for a [`NodeConfig`].
///
/// `scope` is `all` (default), `fullnode`, or `validator-<N>`. `path` is a
/// dot-separated list of the kebab-case field names used in the node config
/// YAML, and `value` is parsed as YAML, e.g.
/// `fullnode:authority-store-pruning-config.num-epochs-to-retain=5`.
///
/// List elements cannot be addressed by index; a list can only be replaced as
/// a whole, by passing the new list as the YAML value.
#[derive(Clone, Debug)]
pub struct NodeConfigOverride {
    pub scope: OverrideScope,
    pub path: Vec<String>,
    pub value: Value,
}

impl NodeConfigOverride {
    pub fn applies_to_validator(&self, index: usize) -> bool {
        matches!(self.scope, OverrideScope::All) || self.scope == OverrideScope::Validator(index)
    }

    pub fn applies_to_fullnode(&self) -> bool {
        matches!(self.scope, OverrideScope::All | OverrideScope::Fullnode)
    }

    /// Whether the override sets the given config field, or something that
    /// contains it.
    fn targets(&self, field_path: &[&str]) -> bool {
        !self.path.is_empty()
            && self
                .path
                .iter()
                .zip(field_path)
                .all(|(segment, field)| segment == field)
    }

    /// Set `path` to `value` in `config`.
    ///
    /// The config is serialized to YAML, patched, and deserialized back, so
    /// type mismatches and paths serde does not recognize are rejected.
    pub fn apply_to(&self, config: &mut NodeConfig) -> anyhow::Result<()> {
        let mut root = serde_yaml::to_value(&*config).context("failed to serialize node config")?;
        let mut cursor = &mut root;
        for (i, segment) in self.path.iter().enumerate() {
            // An absent optional sub-config serializes as null; create the
            // mapping so a nested field of it can still be set.
            if cursor.is_null() {
                *cursor = Value::Mapping(Mapping::new());
            }
            let mapping = cursor.as_mapping_mut().ok_or_else(|| {
                anyhow!(
                    "`{}` in `{self}` does not refer to a config section",
                    self.path[..i].join(".")
                )
            })?;
            let key = Value::from(segment.as_str());
            if !mapping.contains_key(&key) {
                mapping.insert(key.clone(), Value::Null);
            }
            cursor = mapping.get_mut(&key).expect("key inserted above");
        }
        *cursor = self.value.clone();

        // `supported_protocol_versions` is #[serde(skip)] on NodeConfig and
        // would be lost in the round trip; preserve it.
        let supported_protocol_versions = config.supported_protocol_versions;
        // These fields are omitted when `None` but deserialize to a non-`None`
        // default, so the round trip would set them behind the user's back.
        // `apply_leaves_untouched_fields_unchanged` guards the list.
        let policy_config = config.policy_config.clone();
        let grpc_api_config = config.grpc_api_config.clone();
        let periodic_compaction_threshold_days = config
            .authority_store_pruning_config
            .periodic_compaction_threshold_days;
        *config = serde_yaml::from_value(root).with_context(|| format!("invalid `{self}`"))?;
        config.supported_protocol_versions = supported_protocol_versions;
        if !self.targets(&["policy-config"]) {
            config.policy_config = policy_config;
        }
        if !self.targets(&["grpc-api-config"]) {
            config.grpc_api_config = grpc_api_config;
        }
        if !self.targets(&[
            "authority-store-pruning-config",
            "periodic-compaction-threshold-days",
        ]) {
            config
                .authority_store_pruning_config
                .periodic_compaction_threshold_days = periodic_compaction_threshold_days;
        }

        // Serde ignores unknown fields, so verify the path survived the round
        // trip to catch typos. Null is exempt: a cleared Option is not
        // serialized.
        if !self.value.is_null() {
            let reserialized =
                serde_yaml::to_value(&*config).context("failed to serialize node config")?;
            let mut cursor = &reserialized;
            for segment in &self.path {
                cursor = cursor.get(segment).ok_or_else(|| {
                    anyhow!(
                        "`{}` is not a known node config field (in `{self}`)",
                        self.path.join(".")
                    )
                })?;
            }
        }
        Ok(())
    }
}

impl FromStr for NodeConfigOverride {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (target, value) = s
            .split_once('=')
            .ok_or_else(|| anyhow!("expected `[scope:]<path>=<value>`, got `{s}`"))?;
        let (scope, path) = match target.split_once(':') {
            None => (OverrideScope::All, target),
            Some(("all", path)) => (OverrideScope::All, path),
            Some(("fullnode", path)) => (OverrideScope::Fullnode, path),
            Some((scope, path)) => {
                let index = scope
                    .strip_prefix("validator-")
                    .and_then(|index| index.parse().ok())
                    .ok_or_else(|| {
                        anyhow!("invalid scope `{scope}`, expected `all`, `fullnode`, or `validator-<N>`")
                    })?;
                (OverrideScope::Validator(index), path)
            }
        };
        let path: Vec<String> = path.split('.').map(str::to_owned).collect();
        if path.iter().any(String::is_empty) {
            bail!("invalid config path `{path}`", path = path.join("."));
        }
        if PROTECTED_FIELDS.contains(&path[0].as_str()) {
            bail!("`{}` cannot be overridden", path[0]);
        }
        let value =
            serde_yaml::from_str(value).with_context(|| format!("invalid YAML value `{value}`"))?;
        Ok(Self { scope, path, value })
    }
}

impl fmt::Display for NodeConfigOverride {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Kept to a single line so it reads well inside an error message.
        let value = serde_yaml::to_string(&self.value).map_or_else(
            |_| format!("{:?}", self.value),
            |value| value.trim_end().replace('\n', " "),
        );
        write!(f, "{}:{}={value}", self.scope, self.path.join("."))
    }
}

#[cfg(test)]
mod tests {
    use iota_config::node::Genesis;
    use rand::rngs::OsRng;

    use super::*;
    use crate::node_config_builder::FullnodeConfigBuilder;

    fn test_config() -> NodeConfig {
        FullnodeConfigBuilder::new().build_from_parts(&mut OsRng, &[], Genesis::new_empty())
    }

    #[test]
    fn parse_scopes() {
        let all: NodeConfigOverride = "enable-index-processing=false".parse().unwrap();
        assert_eq!(all.scope, OverrideScope::All);
        assert_eq!(all.path, ["enable-index-processing"]);
        assert_eq!(all.value, Value::Bool(false));

        let all: NodeConfigOverride = "all:enable-index-processing=false".parse().unwrap();
        assert_eq!(all.scope, OverrideScope::All);

        let fullnode: NodeConfigOverride =
            "fullnode:authority-store-pruning-config.num-epochs-to-retain=18446744073709551615"
                .parse()
                .unwrap();
        assert_eq!(fullnode.scope, OverrideScope::Fullnode);
        assert_eq!(
            fullnode.path,
            ["authority-store-pruning-config", "num-epochs-to-retain"]
        );
        assert_eq!(fullnode.value, Value::from(u64::MAX));

        let validator: NodeConfigOverride =
            "validator-2:enable-soft-locking=false".parse().unwrap();
        assert_eq!(validator.scope, OverrideScope::Validator(2));
    }

    #[test]
    fn parse_rejects_malformed_input() {
        for input in [
            "no-equals-sign",
            "bad-scope:foo=1",
            "validator-x:foo=1",
            "=1",
            "a..b=1",
            "genesis=whatever",
            "authority-key-pair=secret",
        ] {
            assert!(input.parse::<NodeConfigOverride>().is_err(), "{input}");
        }
    }

    #[test]
    fn apply_sets_nested_value() {
        let mut config = test_config();
        assert_eq!(
            config.authority_store_pruning_config.num_epochs_to_retain,
            0
        );

        let config_override: NodeConfigOverride =
            "authority-store-pruning-config.num-epochs-to-retain=18446744073709551615"
                .parse()
                .unwrap();
        config_override.apply_to(&mut config).unwrap();
        assert_eq!(
            config.authority_store_pruning_config.num_epochs_to_retain,
            u64::MAX
        );
    }

    #[test]
    fn apply_preserves_supported_protocol_versions() {
        use iota_types::supported_protocol_versions::SupportedProtocolVersions;

        let mut config = test_config();
        config.supported_protocol_versions = Some(SupportedProtocolVersions::SYSTEM_DEFAULT);

        let config_override: NodeConfigOverride = "enable-index-processing=false".parse().unwrap();
        config_override.apply_to(&mut config).unwrap();
        assert_eq!(
            config.supported_protocol_versions,
            Some(SupportedProtocolVersions::SYSTEM_DEFAULT)
        );
    }

    #[test]
    fn apply_leaves_untouched_fields_unchanged() {
        // The lazily loaded key pair caches are not config state and start out
        // empty on a freshly deserialized config, so fill them in first.
        fn debug_with_keys_loaded(config: &NodeConfig) -> String {
            config.authority_key_pair();
            config.protocol_key_pair();
            config.network_key_pair();
            config.iota_address();
            format!("{config:?}")
        }

        let mut config = test_config();
        let before = debug_with_keys_loaded(&config);
        let enable_index_processing = config.enable_index_processing;

        let config_override: NodeConfigOverride = "enable-index-processing=false".parse().unwrap();
        config_override.apply_to(&mut config).unwrap();
        // Undo the single intended change; nothing else may have moved.
        config.enable_index_processing = enable_index_processing;

        assert_eq!(debug_with_keys_loaded(&config), before);
    }

    #[test]
    fn apply_null_clears_optional_field() {
        use iota_config::node::MetricsConfig;

        let mut config = test_config();
        config.metrics = Some(MetricsConfig {
            push_interval_seconds: Some(10),
            push_url: None,
            groups: None,
        });

        let config_override: NodeConfigOverride = "metrics=null".parse().unwrap();
        config_override.apply_to(&mut config).unwrap();
        assert!(config.metrics.is_none());
    }

    #[test]
    fn apply_creates_absent_optional_section() {
        let mut config = test_config();
        assert!(config.metrics.is_none());

        let config_override: NodeConfigOverride =
            "metrics.push-interval-seconds=10".parse().unwrap();
        config_override.apply_to(&mut config).unwrap();
        assert_eq!(config.metrics.unwrap().push_interval_seconds, Some(10));
    }

    #[test]
    fn apply_rejects_unknown_field() {
        let mut config = test_config();
        let config_override: NodeConfigOverride =
            "authority-store-pruning-config.num-epochs-to-retan=5"
                .parse()
                .unwrap();
        assert!(config_override.apply_to(&mut config).is_err());
    }

    #[test]
    fn apply_rejects_nesting_under_a_scalar() {
        let mut config = test_config();
        let config_override: NodeConfigOverride = "enable-index-processing.foo=1".parse().unwrap();
        let err = config_override
            .apply_to(&mut config)
            .unwrap_err()
            .to_string();
        assert!(
            err.starts_with("`enable-index-processing` in "),
            "error blames the wrong segment: {err}"
        );
    }

    #[test]
    fn apply_rejects_type_mismatch() {
        let mut config = test_config();
        let config_override: NodeConfigOverride =
            "authority-store-pruning-config.num-epochs-to-retain=not-a-number"
                .parse()
                .unwrap();
        assert!(config_override.apply_to(&mut config).is_err());
    }
}
