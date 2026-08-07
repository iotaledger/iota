// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Generic key-value overrides applied to generated [`NodeConfig`]s.

use std::{fmt, str::FromStr};

use anyhow::{Context, anyhow, bail, ensure};
use iota_config::NodeConfig;
use serde_yaml::{Mapping, Value};

/// Node config fields that must not be overridden because they carry node
/// identity or data that has to stay consistent across the network.
///
/// All of them are top-level [`NodeConfig`] fields without serde aliases, so
/// checking the first path segment of an override is sufficient.
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
    /// All validators.
    AllValidators,
    /// The validator at the given index in the network config.
    Validator(usize),
}

impl fmt::Display for OverrideScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OverrideScope::All => write!(f, "all"),
            OverrideScope::Fullnode => write!(f, "fullnode"),
            OverrideScope::AllValidators => write!(f, "validator"),
            OverrideScope::Validator(index) => write!(f, "validator-{index}"),
        }
    }
}

/// A single `[scope:]<path>=<value>` override for a [`NodeConfig`].
///
/// `scope` is `all` (default), `fullnode`, `validator` (every validator), or
/// `validator-<N>`. `path` is a dot-separated list of the field names as they
/// appear in the node config YAML (kebab-case for most sections), and `value`
/// is parsed as YAML, e.g.
/// `fullnode:authority-store-pruning-config.num-epochs-to-retain=5`.
///
/// Because the value is YAML, values that would otherwise parse as YAML
/// structure need quoting (e.g. `'[::1]:9000'`), and `null` or an empty
/// value clears an optional field.
///
/// A mapping value merges with the section's current fields, so unmentioned
/// fields keep their values. List elements cannot be addressed by index; a
/// list can only be replaced as a whole, by passing the new list as the YAML
/// value.
#[derive(Clone, Debug)]
pub struct NodeConfigOverride {
    pub scope: OverrideScope,
    path: Vec<String>,
    value: Value,
}

/// Fail if a `validator-<N>` scoped override names a validator the network
/// does not have.
pub fn check_validator_override_scopes(
    node_config_overrides: &[NodeConfigOverride],
    num_validators: usize,
) -> anyhow::Result<()> {
    for config_override in node_config_overrides {
        if let OverrideScope::Validator(index) = config_override.scope {
            let noun = if num_validators == 1 {
                "validator"
            } else {
                "validators"
            };
            ensure!(
                index < num_validators,
                "`{config_override}` targets validator {index}, but the network has only \
                 {num_validators} {noun}"
            );
        }
    }
    Ok(())
}

impl NodeConfigOverride {
    pub fn applies_to_validator(&self, index: usize) -> bool {
        matches!(
            self.scope,
            OverrideScope::All | OverrideScope::AllValidators
        ) || self.scope == OverrideScope::Validator(index)
    }

    pub fn applies_to_fullnode(&self) -> bool {
        matches!(self.scope, OverrideScope::All | OverrideScope::Fullnode)
    }

    /// Whether the override sets the given config field: it names the field
    /// itself or something inside it, or it replaces an enclosing section
    /// with a value that mentions the field.
    fn targets(&self, field_path: &[&str]) -> bool {
        if self.path.is_empty()
            || !self
                .path
                .iter()
                .zip(field_path)
                .all(|(segment, field)| segment == field)
        {
            return false;
        }
        if self.path.len() >= field_path.len() {
            return true;
        }
        // The override replaces an enclosing section; it sets the field only
        // if the given value mentions it.
        field_path[self.path.len()..]
            .iter()
            .try_fold(&self.value, |value, segment| value.get(*segment))
            .is_some()
    }

    /// Set `path` to `value` in `config`.
    ///
    /// Unknown fields, type mismatches, and changes that would leave the
    /// node unable to start are rejected; on error the config is left
    /// unchanged.
    pub fn apply_to(&self, config: &mut NodeConfig) -> anyhow::Result<()> {
        // iota-node and iota-swarm decide whether a node runs as a validator
        // or a fullnode by whether it has a consensus config, so an override
        // must not create or remove that section.
        if config.consensus_config.is_none()
            && self.path.first().is_some_and(|s| s == "consensus-config")
            && !(self.path.len() == 1 && self.value.is_null())
        {
            bail!(
                "`{self}` would create `consensus-config` on a fullnode; only validators have a \
                 consensus config, use a validator scope"
            );
        }

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
        merge_value(cursor, self.value.clone());

        // Serde silently drops unknown fields, so collect what it ignores to
        // catch typos anywhere in the path or the value.
        let mut ignored_fields = Vec::new();
        let mut new_config: NodeConfig = serde_ignored::deserialize(root, |field| {
            ignored_fields.push(ignored_field_path(&field))
        })
        .with_context(|| format!("invalid `{self}`"))?;
        let path = self.path.join(".");
        if let Some(unknown) = ignored_fields.iter().find(|field| {
            field.as_str() == path
                || field.starts_with(&format!("{path}."))
                || path.starts_with(&format!("{field}."))
        }) {
            bail!("`{unknown}` is not a known node config field (in `{self}`)");
        }

        if config.consensus_config.is_some() && new_config.consensus_config.is_none() {
            bail!(
                "`{self}` would remove `consensus-config` from a validator, which cannot run \
                 without it"
            );
        }

        // `supported_protocol_versions` is #[serde(skip)] on NodeConfig and
        // would be lost in the round trip; preserve it.
        new_config.supported_protocol_versions = config.supported_protocol_versions;
        // These fields are omitted when `None` but deserialize to a non-`None`
        // default, so the round trip would set them behind the user's back.
        // Keep the old value unless the override needs the deserialized
        // default in place: a newly created firewall config only takes effect
        // alongside a policy config, and an enabled gRPC API needs a config.
        // `apply_leaves_untouched_fields_unchanged` guards the list.
        let firewall_config_created =
            config.firewall_config.is_none() && new_config.firewall_config.is_some();
        if !(self.targets(&["policy-config"]) || firewall_config_created) {
            new_config.policy_config = config.policy_config.clone();
        }
        if !(self.targets(&["grpc-api-config"])
            || (self.targets(&["enable-grpc-api"]) && new_config.enable_grpc_api))
        {
            new_config.grpc_api_config = config.grpc_api_config.clone();
        }
        if !self.targets(&[
            "authority-store-pruning-config",
            "periodic-compaction-threshold-days",
        ]) {
            new_config
                .authority_store_pruning_config
                .periodic_compaction_threshold_days = config
                .authority_store_pruning_config
                .periodic_compaction_threshold_days;
        }

        if new_config.enable_grpc_api && new_config.grpc_api_config.is_none() {
            bail!(
                "`{self}` leaves the gRPC API enabled without a `grpc-api-config`, a state the \
                 node refuses to start in"
            );
        }

        *config = new_config;
        Ok(())
    }
}

/// Merge `value` into `target`: mappings merge recursively so unmentioned
/// fields keep their current values; any other value replaces the old one.
fn merge_value(target: &mut Value, value: Value) {
    match (target, value) {
        (Value::Mapping(target), Value::Mapping(value)) => {
            for (key, value) in value {
                match target.get_mut(&key) {
                    Some(existing) => merge_value(existing, value),
                    None => {
                        target.insert(key, value);
                    }
                }
            }
        }
        (target, value) => *target = value,
    }
}

/// Dot-joined path of a field serde ignored, without the `?` markers
/// serde_ignored inserts for each `Option` and newtype layer it descends
/// through.
///
/// serde_ignored cannot see into `#[serde(flatten)]` content; every
/// flattened field in the `NodeConfig` tree is protected, so no override
/// path reaches one.
fn ignored_field_path(path: &serde_ignored::Path<'_>) -> String {
    use serde_ignored::Path;
    let (parent, segment) = match path {
        Path::Root => return String::new(),
        Path::Seq { parent, index } => (parent, index.to_string()),
        Path::Map { parent, key } => (parent, key.clone()),
        Path::Some { parent }
        | Path::NewtypeStruct { parent }
        | Path::NewtypeVariant { parent } => return ignored_field_path(parent),
    };
    let parent = ignored_field_path(parent);
    if parent.is_empty() {
        segment
    } else {
        format!("{parent}.{segment}")
    }
}

impl FromStr for NodeConfigOverride {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (target, value) = s
            .trim()
            .split_once('=')
            .ok_or_else(|| anyhow!("expected `[scope:]<path>=<value>`, got `{s}`"))?;
        let (scope, path) = match target.trim().split_once(':') {
            None => (OverrideScope::All, target.trim()),
            Some((scope, path)) => {
                let (scope, path) = (scope.trim(), path.trim());
                match scope {
                    "all" => (OverrideScope::All, path),
                    "fullnode" => (OverrideScope::Fullnode, path),
                    "validator" => (OverrideScope::AllValidators, path),
                    _ => {
                        let index = scope
                            .strip_prefix("validator-")
                            // Only the canonical spelling: a sign or leading
                            // zeros would alias another scope's display form.
                            .and_then(|index| {
                                let parsed: usize = index.parse().ok()?;
                                (parsed.to_string() == index).then_some(parsed)
                            })
                            .ok_or_else(|| {
                                anyhow!(
                                    "invalid scope `{scope}`, expected `all`, `fullnode`, \
                                     `validator`, or `validator-<N>`"
                                )
                            })?;
                        (OverrideScope::Validator(index), path)
                    }
                }
            }
        };
        let path: Vec<String> = path
            .split('.')
            .map(|segment| segment.trim().to_owned())
            .collect();
        if path.iter().any(String::is_empty) {
            bail!(
                "invalid config path `{path}` (expected `[scope:]<path>=<value>`)",
                path = path.join(".")
            );
        }
        if PROTECTED_FIELDS.contains(&path[0].as_str()) {
            bail!("`{}` cannot be overridden", path[0]);
        }
        // An empty value means null, like an empty value in a YAML file.
        let value = if value.trim().is_empty() {
            Value::Null
        } else {
            serde_yaml::from_str(value)
                .map_err(|err| anyhow!("invalid YAML value `{value}`: {err}"))?
        };
        Ok(Self { scope, path, value })
    }
}

impl fmt::Display for NodeConfigOverride {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Kept to a single line so it reads well inside an error message;
        // serde_yaml starts every document with a `---` marker, drop it.
        let value = serde_yaml::to_string(&self.value).map_or_else(
            |_| format!("{:?}", self.value),
            |value| {
                value
                    .strip_prefix("---")
                    .unwrap_or(&value)
                    .trim()
                    .replace('\n', " ")
            },
        );
        write!(f, "{}:{}={value}", self.scope, self.path.join("."))
    }
}

#[cfg(test)]
mod tests {
    use iota_config::node::Genesis;
    use rand::rngs::OsRng;

    use super::*;
    use crate::{
        genesis_config::ValidatorGenesisConfigBuilder,
        node_config_builder::{FullnodeConfigBuilder, ValidatorConfigBuilder},
    };

    fn test_config() -> NodeConfig {
        FullnodeConfigBuilder::new().build_from_parts(&mut OsRng, &[], Genesis::new_empty())
    }

    fn validator_test_config() -> NodeConfig {
        ValidatorConfigBuilder::new()
            .build_without_genesis(ValidatorGenesisConfigBuilder::new().build(&mut OsRng))
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
            "validator-007:foo=1",
            "validator-+3:foo=1",
            "=1",
            "a..b=1",
        ] {
            assert!(input.parse::<NodeConfigOverride>().is_err(), "{input}");
        }
    }

    #[test]
    fn parse_rejects_protected_fields() {
        for input in ["genesis=whatever", "authority-key-pair=secret"] {
            assert!(input.parse::<NodeConfigOverride>().is_err(), "{input}");
        }
    }

    #[test]
    fn protected_fields_exist_on_node_config() {
        let root = serde_yaml::to_value(test_config()).unwrap();
        let mapping = root.as_mapping().unwrap();
        for field in PROTECTED_FIELDS {
            assert!(mapping.contains_key(&Value::from(*field)), "{field}");
        }
    }

    #[test]
    fn check_validator_override_scopes_rejects_out_of_range_indexes() {
        let config_override: NodeConfigOverride =
            "validator-5:enable-soft-locking=false".parse().unwrap();
        let err = check_validator_override_scopes(std::slice::from_ref(&config_override), 1)
            .unwrap_err()
            .to_string();
        assert!(err.contains("validator 5"), "{err}");
        check_validator_override_scopes(&[config_override], 6).unwrap();
    }

    #[test]
    fn parse_trims_whitespace_and_reads_empty_value_as_null() {
        let config_override: NodeConfigOverride =
            " enable-index-processing = false ".parse().unwrap();
        assert_eq!(config_override.path, ["enable-index-processing"]);
        assert_eq!(config_override.value, Value::Bool(false));

        let config_override: NodeConfigOverride = "metrics=".parse().unwrap();
        assert!(config_override.value.is_null());
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

    // The lazily loaded key pair caches are not config state and start out
    // empty on a freshly deserialized config, so fill them in first.
    fn debug_with_keys_loaded(config: &NodeConfig) -> String {
        config.authority_key_pair();
        config.protocol_key_pair();
        config.network_key_pair();
        config.iota_address();
        format!("{config:?}")
    }

    #[test]
    fn apply_leaves_untouched_fields_unchanged() {
        for mut config in [test_config(), validator_test_config()] {
            let before = debug_with_keys_loaded(&config);
            let enable_index_processing = config.enable_index_processing;

            let config_override: NodeConfigOverride =
                "enable-index-processing=false".parse().unwrap();
            config_override.apply_to(&mut config).unwrap();
            // Undo the single intended change; nothing else may have moved.
            config.enable_index_processing = enable_index_processing;

            assert_eq!(debug_with_keys_loaded(&config), before);
        }
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
    fn apply_rejects_unknown_or_mistyped_fields() {
        for input in [
            // A typo in the path.
            "authority-store-pruning-config.num-epochs-to-retan=5",
            // A typo with a null value.
            "metrcis=null",
            // A typo under an optional section.
            "metrics.push-intrval-seconds=10",
            // A value of the wrong type.
            "authority-store-pruning-config.num-epochs-to-retain=not-a-number",
        ] {
            let mut config = test_config();
            let config_override: NodeConfigOverride = input.parse().unwrap();
            assert!(config_override.apply_to(&mut config).is_err(), "{input}");
        }

        // A typo under the validator-only consensus section.
        let mut config = validator_test_config();
        let config_override: NodeConfigOverride =
            "consensus-config.db-retention-epocs=2".parse().unwrap();
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
    fn grpc_api_config_stays_consistent_with_enable_grpc_api() {
        let mut config = test_config();
        assert!(!config.enable_grpc_api);
        assert!(config.grpc_api_config.is_none());

        // Disabling the already disabled API must not create a config.
        let disable: NodeConfigOverride = "enable-grpc-api=false".parse().unwrap();
        disable.apply_to(&mut config).unwrap();
        assert!(config.grpc_api_config.is_none());

        // Clearing the config is fine while the API is disabled.
        let clear: NodeConfigOverride = "grpc-api-config=null".parse().unwrap();
        clear.apply_to(&mut config).unwrap();
        assert!(config.grpc_api_config.is_none());

        // Enabling the API keeps the serde-default config; without one the
        // node refuses to start.
        let enable: NodeConfigOverride = "enable-grpc-api=true".parse().unwrap();
        enable.apply_to(&mut config).unwrap();
        assert!(config.enable_grpc_api);
        assert!(config.grpc_api_config.is_some());

        // Clearing the config while the API is enabled is rejected.
        assert!(clear.apply_to(&mut config).is_err());
        assert!(config.grpc_api_config.is_some());
    }

    #[test]
    fn policy_config_follows_firewall_config_overrides() {
        // The firewall only takes effect alongside a policy config, so
        // setting one keeps the serde-default policy config.
        let mut config = test_config();
        assert!(config.policy_config.is_none());
        let set: NodeConfigOverride =
            "firewall-config={remote-fw-url: 'http://127.0.0.1:65000', destination-port: 65000}"
                .parse()
                .unwrap();
        set.apply_to(&mut config).unwrap();
        assert!(config.firewall_config.is_some());
        assert!(config.policy_config.is_some());

        // Clearing the firewall must not switch the policy on.
        let mut config = test_config();
        let clear: NodeConfigOverride = "firewall-config=null".parse().unwrap();
        clear.apply_to(&mut config).unwrap();
        assert!(config.firewall_config.is_none());
        assert!(config.policy_config.is_none());

        // An edit to an existing firewall config must not resurrect a policy
        // config that was explicitly cleared.
        let mut config = test_config();
        set.apply_to(&mut config).unwrap();
        let clear_policy: NodeConfigOverride = "policy-config=null".parse().unwrap();
        clear_policy.apply_to(&mut config).unwrap();
        assert!(config.policy_config.is_none());
        let edit: NodeConfigOverride = "firewall-config.destination-port=65001".parse().unwrap();
        edit.apply_to(&mut config).unwrap();
        assert_eq!(config.firewall_config.unwrap().destination_port, 65001);
        assert!(config.policy_config.is_none());
    }

    #[test]
    fn apply_accepts_values_omitted_by_skip_serializing_if() {
        // `false` and `[]` are omitted from the serialized config by
        // `skip_serializing_if`, so overriding to them must not be mistaken
        // for an unknown field.
        let mut config = test_config();
        config
            .authority_store_pruning_config
            .enable_compaction_filter = true;
        let config_override: NodeConfigOverride =
            "authority-store-pruning-config.enable-compaction-filter=false"
                .parse()
                .unwrap();
        config_override.apply_to(&mut config).unwrap();
        assert!(
            !config
                .authority_store_pruning_config
                .enable_compaction_filter
        );

        let config_override: NodeConfigOverride = "p2p-config.seed-peers=[]".parse().unwrap();
        config_override.apply_to(&mut config).unwrap();
        assert!(config.p2p_config.seed_peers.is_empty());
    }

    #[test]
    fn section_override_merges_with_existing_fields() {
        let mut config = test_config();
        // What --disable-fullnode-pruning sets on the built config.
        config.authority_store_pruning_config.num_epochs_to_retain = u64::MAX;

        let config_override: NodeConfigOverride =
            "authority-store-pruning-config={enable-compaction-filter: true}"
                .parse()
                .unwrap();
        config_override.apply_to(&mut config).unwrap();
        assert!(
            config
                .authority_store_pruning_config
                .enable_compaction_filter
        );
        // Unmentioned fields keep their values instead of resetting to their
        // serde defaults.
        assert_eq!(
            config.authority_store_pruning_config.num_epochs_to_retain,
            u64::MAX
        );
        assert_eq!(
            config
                .authority_store_pruning_config
                .num_latest_epoch_dbs_to_retain,
            3
        );
    }

    #[test]
    fn whole_section_override_sets_only_fields_the_value_mentions() {
        let mut config = test_config();
        assert_eq!(
            config
                .authority_store_pruning_config
                .periodic_compaction_threshold_days,
            None
        );

        // The unmentioned field keeps its value instead of taking the serde
        // field default `Some(1)`.
        let config_override: NodeConfigOverride =
            "authority-store-pruning-config={num-epochs-to-retain: 5}"
                .parse()
                .unwrap();
        config_override.apply_to(&mut config).unwrap();
        assert_eq!(
            config.authority_store_pruning_config.num_epochs_to_retain,
            5
        );
        assert_eq!(
            config
                .authority_store_pruning_config
                .periodic_compaction_threshold_days,
            None
        );

        // A field the value mentions is set.
        let config_override: NodeConfigOverride =
            "authority-store-pruning-config={periodic-compaction-threshold-days: 7}"
                .parse()
                .unwrap();
        config_override.apply_to(&mut config).unwrap();
        assert_eq!(
            config
                .authority_store_pruning_config
                .periodic_compaction_threshold_days,
            Some(7)
        );
    }

    #[test]
    fn display_formats_the_override_on_one_line() {
        let config_override: NodeConfigOverride =
            "fullnode:authority-store-pruning-config.num-epochs-to-retain=5"
                .parse()
                .unwrap();
        assert_eq!(
            config_override.to_string(),
            "fullnode:authority-store-pruning-config.num-epochs-to-retain=5"
        );

        let config_override: NodeConfigOverride =
            "validator-2:enable-soft-locking=false".parse().unwrap();
        assert_eq!(
            config_override.to_string(),
            "validator-2:enable-soft-locking=false"
        );

        let config_override: NodeConfigOverride =
            "fullnode:authority-store-pruning-config={num-epochs-to-retain: 5, \
             num-latest-epoch-dbs-to-retain: 9}"
                .parse()
                .unwrap();
        assert_eq!(
            config_override.to_string(),
            "fullnode:authority-store-pruning-config=num-epochs-to-retain: 5 \
             num-latest-epoch-dbs-to-retain: 9"
        );
    }

    #[test]
    fn failed_apply_leaves_config_unchanged() {
        let mut config = test_config();
        let before = debug_with_keys_loaded(&config);
        let config_override: NodeConfigOverride =
            "authority-store-pruning-config={num-epochs-to-retain: 5, num-epochs-to-retan: 7}"
                .parse()
                .unwrap();
        assert!(config_override.apply_to(&mut config).is_err());
        assert_eq!(debug_with_keys_loaded(&config), before);
    }

    #[test]
    fn apply_keeps_consensus_config_presence() {
        // Creating the validator-only section on a fullnode is rejected...
        let mut config = test_config();
        let create: NodeConfigOverride = "consensus-config.db-retention-epochs=2".parse().unwrap();
        let err = create.apply_to(&mut config).unwrap_err().to_string();
        assert!(err.contains("validator"), "unhelpful error: {err}");
        assert!(config.consensus_config.is_none());

        // ...and so is removing it from a validator.
        let mut config = validator_test_config();
        let clear: NodeConfigOverride = "consensus-config=null".parse().unwrap();
        assert!(clear.apply_to(&mut config).is_err());
        assert!(config.consensus_config.is_some());
    }

    #[test]
    fn parse_validator_scope_covers_all_validators() {
        let config_override: NodeConfigOverride =
            "validator:enable-soft-locking=false".parse().unwrap();
        assert_eq!(config_override.scope, OverrideScope::AllValidators);
        assert!(config_override.applies_to_validator(3));
        assert!(!config_override.applies_to_fullnode());
        assert_eq!(
            config_override.to_string(),
            "validator:enable-soft-locking=false"
        );
    }
}
