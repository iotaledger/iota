// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Generic key-value overrides applied to generated [`NodeConfig`]s.

use std::{fmt, str::FromStr};

use anyhow::{Context, anyhow, bail, ensure};
use iota_config::NodeConfig;
use serde_yaml::{Mapping, Value};

/// The key pair fields of a [`NodeConfig`]: they carry node identity and
/// must not be overridden.
const KEY_PAIR_FIELDS: &[&str] = &[
    "authority-key-pair",
    "protocol-key-pair",
    "account-key-pair",
    "network-key-pair",
];

/// Further fields that must not be overridden because they have to stay
/// consistent across the network.
///
/// These and [`KEY_PAIR_FIELDS`] are top-level [`NodeConfig`] fields without
/// serde aliases, so checking the first path segment of an override is
/// sufficient.
const PROTECTED_FIELDS: &[&str] = &["genesis", "migration-tx-data-path"];

/// The addresses `ValidatorGenesisConfig::to_validator_info` copies into
/// `ValidatorInfo`, and therefore into the committee metadata in
/// `genesis.blob`. Each is paired with the [`NodeConfig`] path it is
/// written to.
///
/// A validator whose config disagrees with its committee entry cannot be
/// reached at the address its peers dial. These addresses therefore change
/// only by re-running genesis.
///
/// `primary-address` has no [`NodeConfig`] field of its own, because a
/// validator reads its peers' primary addresses from the committee. It is
/// named here so that an override of it gets the reason instead of
/// "not a known node config field".
///
/// A fullnode is not a committee member, so none of these are genesis data
/// for it and a `fullnode:` scoped override of them is allowed.
const COMMITTEE_ADDRESS_FIELDS: &[(&str, &str)] = &[
    ("network-address", "network-address"),
    ("p2p-config.external-address", "p2p-address"),
    ("primary-address", "primary-address"),
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

impl OverrideScope {
    /// Whether the scope reaches at least one validator.
    fn reaches_a_validator(self) -> bool {
        !matches!(self, OverrideScope::Fullnode)
    }
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
/// `scope` is `all` (default), `fullnode`, `validator` (all validators), or
/// `validator-<N>`.
///
/// `path` is a dot-separated list of field names as they appear in the
/// config YAML.
///
/// `value` is YAML. `null`, or an empty value, clears a field. If the field
/// is already unset, the clear changes nothing. A mapping merges with the
/// section's current fields. A list replaces the current list as a whole.
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
                "`{}` targets validator {index}, but the network has only {num_validators} {noun}",
                config_override.scoped_field_path()
            );
        }
    }
    Ok(())
}

/// The overrides that apply to the validator at `index` in the network
/// config, in the given order.
pub fn overrides_for_validator(
    overrides: &[NodeConfigOverride],
    index: usize,
) -> impl Iterator<Item = &NodeConfigOverride> {
    overrides
        .iter()
        .filter(move |config_override| config_override.applies_to_validator(index))
}

/// The overrides that apply to a fullnode, in the given order.
pub fn overrides_for_fullnode(
    overrides: &[NodeConfigOverride],
) -> impl Iterator<Item = &NodeConfigOverride> {
    overrides
        .iter()
        .filter(|config_override| config_override.applies_to_fullnode())
}

/// The fields the given overrides set: later overrides shadow earlier
/// entries for the same field and for anything nested inside it.
pub fn winning_field_paths<'a>(
    overrides: impl IntoIterator<Item = &'a NodeConfigOverride>,
) -> Vec<String> {
    let mut fields: Vec<String> = Vec::new();
    for config_override in overrides {
        for field_path in config_override.field_paths() {
            fields
                .retain(|path| *path != field_path && !path.starts_with(&format!("{field_path}.")));
            fields.push(field_path);
        }
    }
    fields
}

impl NodeConfigOverride {
    fn applies_to_validator(&self, index: usize) -> bool {
        matches!(
            self.scope,
            OverrideScope::All | OverrideScope::AllValidators
        ) || self.scope == OverrideScope::Validator(index)
    }

    fn applies_to_fullnode(&self) -> bool {
        matches!(self.scope, OverrideScope::All | OverrideScope::Fullnode)
    }

    /// The override's scope and field path, without its value.
    pub fn scoped_field_path(&self) -> String {
        format!("{}:{}", self.scope, self.path.join("."))
    }

    /// The dot-joined paths of the fields this override sets, one per leaf
    /// of its value. An empty mapping names the section itself.
    fn field_paths(&self) -> Vec<String> {
        fn collect(prefix: String, value: &Value, paths: &mut Vec<String>) {
            match value.as_mapping().filter(|mapping| !mapping.is_empty()) {
                Some(mapping) => {
                    for (key, value) in mapping {
                        let segment = key.as_str().unwrap_or_default();
                        collect(format!("{prefix}.{segment}"), value, paths);
                    }
                }
                None => paths.push(prefix),
            }
        }
        let mut paths = Vec::new();
        collect(self.path.join("."), &self.value, &mut paths);
        paths
    }

    /// Whether an unknown field reported at the dot-joined `field_path`
    /// can have come from this override: the path is at, under, or above
    /// the override's own path.
    fn mentions(&self, field_path: &str) -> bool {
        let path = self.path.join(".");
        field_path == path
            || field_path.starts_with(&format!("{path}."))
            || path.starts_with(&format!("{field_path}."))
    }
}

/// Apply `overrides` to `config` in the given order: later overrides win
/// per field, and fields no override mentions keep their current values.
/// Declaration order decides, not how specific a scope is: a later
/// `validator:` override beats an earlier `validator-0:` one.
///
/// The merge runs on the serialized config. An override that sets a field
/// inside a section the config leaves at its default therefore starts from
/// the section's own per-field serde defaults. It does not start from the
/// value the config holds in memory.
///
/// The two agree for `grpc-api-config`. They differ for `policy-config`,
/// whose [`NodeConfig`] default is the DoS protection policy, while
/// `PolicyConfig`'s field defaults compose to the no-op one. Set the whole
/// section to configure that one.
///
/// A section the config switches off with an explicit `null` is a scalar,
/// so an override that sets a field inside it is rejected. To switch the
/// section back on, set the whole section.
///
/// Unknown fields, type mismatches, and changes that would leave the node
/// unable to run are rejected. On error the config is left unchanged.
/// [`NodeConfig::validate`] judges the final state, so an override may
/// clear a field a later one restores. An override that creates or removes
/// `consensus-config` is rejected even if a later override undoes it.
pub fn apply_node_config_overrides<'a>(
    overrides: impl IntoIterator<Item = &'a NodeConfigOverride>,
    config: &mut NodeConfig,
) -> anyhow::Result<()> {
    let overrides: Vec<&NodeConfigOverride> = overrides.into_iter().collect();
    if overrides.is_empty() {
        // Nothing to apply: skip the round trip entirely so an
        // unoverridden config is untouched.
        return Ok(());
    }

    // iota-node and iota-swarm decide whether a node runs as a validator or
    // a fullnode by whether it has a consensus config. An override must
    // therefore not create or remove that section.
    for config_override in &overrides {
        if config_override
            .path
            .first()
            .is_some_and(|s| s == "consensus-config")
        {
            let clears_whole_section =
                config_override.path.len() == 1 && config_override.value.is_null();
            if config.consensus_config.is_none() && !clears_whole_section {
                bail!(
                    "`{}` would create `consensus-config` on a fullnode; only validators have a \
                     consensus config, use a validator scope",
                    config_override.scoped_field_path()
                );
            }
            if config.consensus_config.is_some() && clears_whole_section {
                bail!(
                    "`{}` would remove `consensus-config` from a validator, which cannot run \
                     without it",
                    config_override.scoped_field_path()
                );
            }
        }
    }

    // The genesis is serialized with the config and deep-copied once per
    // override below. At localnet scale that cost is small enough to accept.
    let mut merged = serde_yaml::to_value(&*config).context("failed to serialize node config")?;

    // Apply and check one override at a time. A typo is then caught even if
    // a later override overwrites the field, and the error names the
    // override at fault.
    let mut checked: Option<NodeConfig> = None;
    for config_override in &overrides {
        let mut cursor = &mut merged;
        let last = config_override.path.len() - 1;
        for (i, segment) in config_override.path.iter().enumerate() {
            let mapping = cursor.as_mapping_mut().ok_or_else(|| {
                anyhow!(
                    "`{}` in `{}` does not refer to a config section",
                    config_override.path[..i].join("."),
                    config_override.scoped_field_path()
                )
            })?;
            let key = Value::from(segment.as_str());
            if !mapping.contains_key(&key) {
                // An unset optional section is omitted from the serialized
                // config. Create it so a nested field can still be set. A
                // section the config carries as `null` is left as it is, so
                // the next segment is rejected as nesting under a scalar.
                let absent = if i < last {
                    Value::Mapping(Mapping::new())
                } else {
                    Value::Null
                };
                mapping.insert(key.clone(), absent);
            }
            cursor = mapping.get_mut(&key).expect("key inserted above");
        }
        merge_value(cursor, config_override.value.clone());

        // Serde silently drops unknown fields, so collect what it ignores
        // to catch typos anywhere in the path or the value.
        let mut ignored_fields = Vec::new();
        checked = Some(
            serde_ignored::deserialize(merged.clone(), |field| {
                ignored_fields.push(ignored_field_path(&field))
            })
            .map_err(|err| {
                let message = err.to_string();
                anyhow!(
                    "invalid `{}`: {}",
                    config_override.scoped_field_path(),
                    safe_deserialization_message(&message)
                )
            })?,
        );
        if let Some(unknown) = ignored_fields
            .iter()
            .find(|field| config_override.mentions(field))
        {
            bail!(
                "`{unknown}` is not a known node config field (in `{}`)",
                config_override.scoped_field_path()
            );
        }
    }

    let mut new_config = checked.expect("the batch is non-empty");

    // `supported_protocol_versions` is #[serde(skip)] on NodeConfig and
    // would be lost in the round trip. Preserve it.
    new_config.supported_protocol_versions = config.supported_protocol_versions;

    // Overrides are named by scope and path only: the rejected state is
    // what needs diagnosing, and echoing every value would print the
    // credentials in the ones that are not at fault.
    new_config.validate().with_context(|| {
        let names = match overrides.as_slice() {
            [config_override] => format!("`{}`", config_override.scoped_field_path()),
            _ => format!(
                "the node config overrides ({})",
                overrides
                    .iter()
                    .map(|config_override| config_override.scoped_field_path())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };
        format!("with {names} applied, no node could run with this config")
    })?;

    *config = new_config;
    Ok(())
}

/// Merge `value` into `target`. Mappings merge recursively, so unmentioned
/// fields keep their current values. Any other value replaces the old one.
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

/// Reject mapping keys that are not strings anywhere in an override
/// value: serde reads a number as a field index, which names no field and
/// renders as a broken path in a list of overridden fields.
fn ensure_string_mapping_keys(value: &Value) -> anyhow::Result<()> {
    if let Value::Mapping(mapping) = value {
        for (key, value) in mapping {
            ensure!(
                key.is_string(),
                "mapping keys in an override value must be strings"
            );
            ensure_string_mapping_keys(value)?;
        }
    }
    Ok(())
}

/// What a deserialization failure says when its own message cannot be
/// shown.
const UNFIT_VALUE: &str = "the value does not fit the field's type";

/// `message`, the message of a deserialization failure, when it provably
/// carries no part of the value, and [`UNFIT_VALUE`] otherwise.
///
/// Serde renders the rejected value into most of its messages, and a
/// custom deserializer can render anything at all. Only the two shapes that
/// name a field of the config schema, and nothing else, are kept.
fn safe_deserialization_message(message: &str) -> &str {
    let field = message
        .strip_prefix("missing field `")
        .or_else(|| message.strip_prefix("duplicate field `"))
        .and_then(|field| field.strip_suffix('`'));
    match field {
        Some(field)
            if !field.is_empty()
                && field
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-') =>
        {
            message
        }
        _ => UNFIT_VALUE,
    }
}

/// Dot-joined path of a field serde ignored, without the `?` markers
/// serde_ignored inserts for each `Option` and newtype layer it descends
/// through.
///
/// serde_ignored cannot see into `#[serde(flatten)]` content. Every
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

/// Field names serde also accepts under a second spelling, with the one
/// the serialized config uses. Only an override path is normalized to that
/// spelling. Inside an override value an alias is not normalized. It
/// collides with the canonical key as a duplicate field when the config
/// already carries that key.
const FIELD_NAME_ALIASES: &[(&str, &str)] = &[("starfish_parameters", "parameters")];

impl FromStr for NodeConfigOverride {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // The input is not echoed: everything after the `=` is the value,
        // and a missing `=` makes the whole input one.
        let (target, value) = s
            .trim()
            .split_once('=')
            .ok_or_else(|| anyhow!("expected `[scope:]<path>=<value>`"))?;
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
            .map(|segment| {
                let segment = segment.trim();
                FIELD_NAME_ALIASES
                    .iter()
                    .find_map(|(alias, field)| (*alias == segment).then_some(*field))
                    .unwrap_or(segment)
                    .to_owned()
            })
            .collect();
        if path.iter().any(String::is_empty) {
            bail!(
                "invalid config path `{path}` (expected `[scope:]<path>=<value>`)",
                path = path.join(".")
            );
        }
        if KEY_PAIR_FIELDS.contains(&path[0].as_str())
            || PROTECTED_FIELDS.contains(&path[0].as_str())
        {
            bail!("`{}` cannot be overridden", path[0]);
        }
        // An empty value means null, like an empty value in a YAML file.
        let value = if value.trim().is_empty() {
            Value::Null
        } else {
            // The value may carry a credential, so the error names the path.
            serde_yaml::from_str(value).map_err(|err| {
                anyhow!(
                    "invalid YAML value for `{path}`: {err}",
                    path = path.join(".")
                )
            })?
        };
        ensure_string_mapping_keys(&value)?;
        let config_override = Self { scope, path, value };
        if scope.reaches_a_validator() {
            // Checked against the fields the override sets, so a section
            // spelling and a section-clearing null are caught like the
            // dotted path.
            let field_paths = config_override.field_paths();
            if let Some((field_path, genesis_field)) =
                COMMITTEE_ADDRESS_FIELDS.iter().find(|(field_path, _)| {
                    field_paths.iter().any(|path| {
                        field_path
                            .strip_prefix(path.as_str())
                            .is_some_and(|rest| rest.is_empty() || rest.starts_with('.'))
                    })
                })
            {
                bail!(
                    "`{field_path}` is the validator's `{genesis_field}` in the committee \
                     metadata of `genesis.blob`, so overriding it at start would leave the \
                     validator unreachable at the address its peers dial; change it by \
                     re-running genesis"
                );
            }
        }
        Ok(config_override)
    }
}

#[cfg(test)]
mod tests {
    use iota_config::node::{Genesis, GrpcApiConfig};
    use iota_types::traffic_control::PolicyConfig;
    use rand::rngs::OsRng;

    use super::*;
    use crate::{
        genesis_config::ValidatorGenesisConfigBuilder,
        node_config_builder::{FullnodeConfigBuilder, ValidatorConfigBuilder},
    };

    impl NodeConfigOverride {
        /// Shorthand for a batch of one, which most tests here apply.
        fn apply_to(&self, config: &mut NodeConfig) -> anyhow::Result<()> {
            apply_node_config_overrides(std::slice::from_ref(self), config)
        }
    }

    fn test_config() -> NodeConfig {
        FullnodeConfigBuilder::new().build_from_parts(&mut OsRng, &[], Genesis::new_empty())
    }

    fn validator_test_config() -> NodeConfig {
        ValidatorConfigBuilder::new()
            .build_without_genesis(ValidatorGenesisConfigBuilder::new().build(&mut OsRng))
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

        let all_validators: NodeConfigOverride =
            "validator:enable-soft-locking=false".parse().unwrap();
        assert_eq!(all_validators.scope, OverrideScope::AllValidators);
        assert!(all_validators.applies_to_validator(3));
        assert!(!all_validators.applies_to_fullnode());
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
    fn fields_that_cannot_be_overridden_exist_on_node_config() {
        let root = serde_yaml::to_value(test_config()).unwrap();
        let mapping = root.as_mapping().unwrap();
        for field in KEY_PAIR_FIELDS.iter().chain(PROTECTED_FIELDS) {
            assert!(mapping.contains_key(&Value::from(*field)), "{field}");
        }
    }

    #[test]
    fn committee_addresses_are_rejected_for_a_scope_that_reaches_a_validator() {
        for path in [
            "network-address",
            "p2p-config.external-address",
            "primary-address",
        ] {
            for scope in ["", "all:", "validator:", "validator-0:"] {
                let input = format!("{scope}{path}=/ip4/127.0.0.1/tcp/9200");
                let err = input.parse::<NodeConfigOverride>().unwrap_err().to_string();
                assert!(err.contains("genesis.blob"), "{input}: {err}");
                assert!(err.contains("re-running genesis"), "{input}: {err}");
            }
            // A fullnode is not a committee member, so the same fields are
            // ordinary config there.
            let input = format!("fullnode:{path}=/ip4/127.0.0.1/tcp/9200");
            assert!(input.parse::<NodeConfigOverride>().is_ok(), "{input}");
        }
    }

    #[test]
    fn committee_addresses_are_rejected_under_a_section_spelling() {
        for assignment in [
            "p2p-config={external-address: /ip4/127.0.0.1/udp/9201}",
            "p2p-config=null",
        ] {
            let input = format!("validator:{assignment}");
            let err = input.parse::<NodeConfigOverride>().unwrap_err().to_string();
            assert!(err.contains("genesis.blob"), "{input}: {err}");
            assert!(err.contains("re-running genesis"), "{input}: {err}");

            let input = format!("fullnode:{assignment}");
            assert!(input.parse::<NodeConfigOverride>().is_ok(), "{input}");
        }
    }

    #[test]
    fn addresses_that_are_not_committee_metadata_stay_overridable() {
        let mut config = validator_test_config();
        let overrides: Vec<NodeConfigOverride> = [
            "validator:metrics-address='127.0.0.1:19202'",
            "validator:admin-interface-address='127.0.0.1:19204'",
            "validator:json-rpc-address='127.0.0.1:19000'",
            "validator:db-path=/tmp/overridden-db",
        ]
        .iter()
        .map(|input| input.parse().unwrap())
        .collect();
        apply_node_config_overrides(&overrides, &mut config).unwrap();
        assert_eq!(config.metrics_address.to_string(), "127.0.0.1:19202");
        assert_eq!(
            config.admin_interface_address.to_string(),
            "127.0.0.1:19204"
        );
        assert_eq!(config.json_rpc_address.to_string(), "127.0.0.1:19000");
        assert_eq!(config.db_path.to_str().unwrap(), "/tmp/overridden-db");
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
        for mut config in [test_config(), validator_test_config()] {
            let before = debug_with_keys_loaded(&config);
            let enable_index_processing = config.enable_index_processing;

            let config_override: NodeConfigOverride =
                "enable-index-processing=false".parse().unwrap();
            config_override.apply_to(&mut config).unwrap();
            // Undo the single intended change. Nothing else may have moved.
            config.enable_index_processing = enable_index_processing;

            assert_eq!(debug_with_keys_loaded(&config), before);
        }
    }

    #[test]
    fn an_empty_mapping_creates_the_section_a_null_clears() {
        let mut config = test_config();
        assert!(config.metrics.is_none());

        let create: NodeConfigOverride = "metrics={}".parse().unwrap();
        create.apply_to(&mut config).unwrap();
        let metrics = config.metrics.as_ref().expect("the section was created");
        assert_eq!(metrics.push_interval_seconds, None);
        assert_eq!(metrics.push_url, None);
        assert!(metrics.groups.is_none());

        let clear: NodeConfigOverride = "metrics=".parse().unwrap();
        clear.apply_to(&mut config).unwrap();
        assert!(config.metrics.is_none());

        // A dotted path creates the section it names as well.
        let create_field: NodeConfigOverride = "metrics.push-interval-seconds=10".parse().unwrap();
        create_field.apply_to(&mut config).unwrap();
        assert_eq!(config.metrics.unwrap().push_interval_seconds, Some(10));
    }

    #[test]
    fn clearing_a_section_the_node_does_not_have_changes_nothing() {
        // This includes the validator-only `consensus-config`, which an
        // override may not create on a fullnode but may clear where it is
        // already absent.
        let mut config = test_config();
        let before = debug_with_keys_loaded(&config);
        let overrides: Vec<NodeConfigOverride> = ["consensus-config=", "metrics="]
            .iter()
            .map(|input| input.parse().unwrap())
            .collect();
        apply_node_config_overrides(&overrides, &mut config).unwrap();
        assert!(config.consensus_config.is_none());
        assert_eq!(debug_with_keys_loaded(&config), before);
    }

    #[test]
    fn a_field_name_alias_is_normalized_to_the_serialized_name() {
        // `consensus-config.parameters` also deserializes from
        // `starfish_parameters`. An edit under the alias must land on the
        // field the config already carries, not next to it.
        let mut config = validator_test_config();
        let set: NodeConfigOverride =
            "consensus-config.parameters={max_headers_per_commit_sync_fetch: 7}"
                .parse()
                .unwrap();
        set.apply_to(&mut config).unwrap();

        let edit: NodeConfigOverride =
            "consensus-config.starfish_parameters={max_headers_per_commit_sync_fetch: 9}"
                .parse()
                .unwrap();
        assert_eq!(edit.scoped_field_path(), "all:consensus-config.parameters");
        edit.apply_to(&mut config).unwrap();
        assert_eq!(
            config
                .consensus_config
                .unwrap()
                .parameters
                .unwrap()
                .max_headers_per_commit_sync_fetch,
            9
        );
    }

    #[test]
    fn an_enum_variant_alias_replaces_a_scalar_variant() {
        // `FreqThreshold` is an alias of the serialized `freq-threshold`.
        let mut config = test_config();
        config.policy_config = Some(PolicyConfig::default());
        let config_override: NodeConfigOverride =
            "policy-config.spam-policy-type={FreqThreshold: {client-threshold: 5}}"
                .parse()
                .unwrap();
        config_override.apply_to(&mut config).unwrap();
        let policy_config = format!("{:?}", config.policy_config.unwrap());
        assert!(policy_config.contains("FreqThreshold"), "{policy_config}");

        // An edit that merges it into a variant the config already spells
        // out under the serialized name is rejected. The two names sit side
        // by side.
        let mut config = test_config();
        config.policy_config = Some(PolicyConfig {
            // The config carries the section, and with it the serialized
            // variant name, only while the field deviates from its default.
            channel_capacity: 42,
            ..PolicyConfig::default_dos_protection_policy()
        });
        let err = config_override
            .apply_to(&mut config)
            .unwrap_err()
            .to_string();
        assert!(err.contains("policy-config.spam-policy-type"), "{err}");
    }

    #[test]
    fn apply_rejects_unknown_or_mistyped_fields() {
        let mut config = test_config();
        for input in [
            // A typo in the path.
            "authority-store-pruning-config.num-epochs-to-retan=5",
            // A typo with a null value.
            "not-a-field=null",
            // A typo under an optional section.
            "metrics.not-a-field=10",
            // A typo in value position under an optional section.
            "metrics={not-a-field: 1}",
            // A value of the wrong type.
            "authority-store-pruning-config.num-epochs-to-retain=not-a-number",
        ] {
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
    fn editing_a_field_of_a_section_the_config_switched_off_is_rejected() {
        // A `None` killswitch field serializes as an explicit `null`, so an
        // edit inside it nests under a scalar. Switching the section back on
        // means setting the whole thing.
        let mut config = test_config();
        config.policy_config = None;
        let before = debug_with_keys_loaded(&config);

        let edit: NodeConfigOverride = "policy-config.dry-run=false".parse().unwrap();
        let err = edit.apply_to(&mut config).unwrap_err().to_string();
        assert!(
            err.starts_with("`policy-config` in `all:policy-config.dry-run`"),
            "{err}"
        );
        assert_eq!(debug_with_keys_loaded(&config), before);

        let set_section: NodeConfigOverride = "policy-config={dry-run: false}".parse().unwrap();
        set_section.apply_to(&mut config).unwrap();
        assert!(!config.policy_config.unwrap().dry_run);
    }

    #[test]
    fn a_policy_config_edit_starts_from_the_sections_own_field_defaults() {
        // `NodeConfig` defaults `policy-config` to the DoS protection policy
        // and serializes that value by omitting the key. An edit inside the
        // section therefore merges onto an absent one. The fields it does not
        // mention come from `PolicyConfig`'s own serde defaults, which compose
        // to the no-op policy instead.
        let mut config = test_config();
        config.policy_config = Some(PolicyConfig::default_dos_protection_policy());

        let edit: NodeConfigOverride = "policy-config.dry-run=false".parse().unwrap();
        edit.apply_to(&mut config).unwrap();

        // The result is what the section alone deserializes to, not the
        // policy the config held before the edit.
        let from_field_defaults: PolicyConfig = serde_yaml::from_str("dry-run: false").unwrap();
        let applied = serde_yaml::to_value(config.policy_config.unwrap()).unwrap();
        assert_eq!(
            applied,
            serde_yaml::to_value(from_field_defaults).unwrap(),
            "the edit did not start from `PolicyConfig`'s own field defaults"
        );
        assert_eq!(applied.get("dry-run"), Some(&Value::Bool(false)));
        // The DoS protection policy the config carried set both policy
        // types and a larger channel. The field defaults set neither.
        assert_eq!(applied.get("spam-policy-type"), Some(&Value::from("NoOp")));
        assert_eq!(applied.get("error-policy-type"), Some(&Value::from("NoOp")));
        assert_eq!(applied.get("channel-capacity"), Some(&Value::from(100)));
    }

    #[test]
    fn a_grpc_api_config_edit_lands_on_the_section_default() {
        // Unlike `PolicyConfig`, every field of `GrpcApiConfig` defaults to
        // the value its `Default` impl gives. An edit inside the section
        // therefore leaves the rest of it at the config's default.
        let mut config = test_config();
        config.grpc_api_config = Some(GrpcApiConfig::default());

        let edit: NodeConfigOverride = "grpc-api-config.max-message-size-bytes=1234"
            .parse()
            .unwrap();
        edit.apply_to(&mut config).unwrap();

        let mut expected = serde_yaml::to_value(GrpcApiConfig::default()).unwrap();
        expected
            .as_mapping_mut()
            .unwrap()
            .insert(Value::from("max-message-size-bytes"), Value::from(1234));
        assert_eq!(
            serde_yaml::to_value(config.grpc_api_config.unwrap()).unwrap(),
            expected
        );
    }

    #[test]
    fn enabling_the_grpc_api_needs_its_config_section() {
        // The API is off on a built fullnode, which leaves `grpc-api-config`
        // an explicit `null`, so turning it on alone leaves the node unable
        // to start.
        let mut config = test_config();
        assert!(!config.enable_grpc_api);
        assert!(config.grpc_api_config.is_none());

        let enable: NodeConfigOverride = "enable-grpc-api=true".parse().unwrap();
        let err = format!("{:#}", enable.apply_to(&mut config).unwrap_err());
        assert!(
            err.contains("`enable-grpc-api` is set but `grpc-api-config` is missing"),
            "{err}"
        );
    }

    #[test]
    fn enabling_the_grpc_api_on_a_validator_does_not_need_a_config() {
        // Validators do not expose the gRPC API, so enabling it there needs
        // no config section.
        let mut config = validator_test_config();
        let enable: NodeConfigOverride = "enable-grpc-api=true".parse().unwrap();
        enable.apply_to(&mut config).unwrap();
        assert!(config.enable_grpc_api);
        assert!(config.grpc_api_config.is_none());
    }

    #[test]
    fn parse_rejects_non_string_mapping_keys() {
        // Serde reads an integer key as a field index, which would render
        // as a broken path in a list of overridden fields.
        for input in ["metrics={0: 5}", "metrics={groups: {0: 5}}"] {
            let err = input.parse::<NodeConfigOverride>().unwrap_err().to_string();
            assert!(err.contains("must be strings"), "{input}: {err}");
        }
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
        config.authority_store_pruning_config.num_epochs_to_retain = u64::MAX;
        config
            .authority_store_pruning_config
            .periodic_compaction_threshold_days = None;

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
        // This includes `periodic-compaction-threshold-days`, whose `None`
        // serializes as an explicit `null` and so survives the round trip.
        assert_eq!(
            config
                .authority_store_pruning_config
                .periodic_compaction_threshold_days,
            None
        );

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
    fn clearing_consensus_config_is_rejected_even_when_recreated() {
        let mut config = validator_test_config();
        let db_path = config.consensus_config.as_ref().unwrap().db_path.clone();

        // A section recreated from a bare mapping would silently reset
        // every field the second override does not mention.
        let overrides: [NodeConfigOverride; 2] = [
            "consensus-config=".parse().unwrap(),
            "consensus-config={db-path: /tmp/replaced}".parse().unwrap(),
        ];
        let err = apply_node_config_overrides(&overrides, &mut config)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("`all:consensus-config` would remove `consensus-config`"),
            "{err}"
        );
        assert_eq!(config.consensus_config.as_ref().unwrap().db_path, db_path);
    }

    #[test]
    fn batched_overrides_apply_in_order() {
        let mut config = test_config();
        let overrides: Vec<NodeConfigOverride> = [
            "authority-store-pruning-config.num-epochs-to-retain=5",
            "enable-index-processing=false",
            "authority-store-pruning-config.num-epochs-to-retain=7",
        ]
        .iter()
        .map(|input| input.parse().unwrap())
        .collect();
        apply_node_config_overrides(&overrides, &mut config).unwrap();
        assert_eq!(
            config.authority_store_pruning_config.num_epochs_to_retain,
            7
        );
        assert!(!config.enable_index_processing);

        // A batch is atomic: one bad override rejects the whole batch,
        // and the error names the override at fault.
        let mut config = test_config();
        let before = debug_with_keys_loaded(&config);
        let overrides: Vec<NodeConfigOverride> = ["enable-index-processing=false", "not-a-field=1"]
            .iter()
            .map(|input| input.parse().unwrap())
            .collect();
        let err = apply_node_config_overrides(&overrides, &mut config)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("`not-a-field` is not a known node config field"),
            "{err}"
        );
        assert!(err.contains("all:not-a-field"), "{err}");
        assert_eq!(debug_with_keys_loaded(&config), before);
    }

    #[test]
    fn an_empty_batch_leaves_the_config_untouched() {
        let mut config = test_config();
        let before = debug_with_keys_loaded(&config);
        apply_node_config_overrides(&[], &mut config).unwrap();
        assert_eq!(debug_with_keys_loaded(&config), before);
    }

    #[test]
    fn a_shadowed_override_is_still_checked() {
        // A later override overwriting the field must not hide an earlier
        // override's typo.
        let mut config = test_config();
        let overrides: Vec<NodeConfigOverride> = ["metrics.not-a-field=1", "metrics="]
            .iter()
            .map(|input| input.parse().unwrap())
            .collect();
        let err = apply_node_config_overrides(&overrides, &mut config)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains(
                "`metrics.not-a-field` is not a known node config field (in \
                 `all:metrics.not-a-field`)"
            ),
            "{err}"
        );

        // An override that nests under a field an earlier override set to a
        // scalar is blamed itself, not the earlier valid one.
        let mut config = test_config();
        let overrides: Vec<NodeConfigOverride> = [
            "enable-index-processing=false",
            "enable-index-processing.foo=1",
        ]
        .iter()
        .map(|input| input.parse().unwrap())
        .collect();
        let err = apply_node_config_overrides(&overrides, &mut config)
            .unwrap_err()
            .to_string();
        assert!(err.contains("all:enable-index-processing.foo"), "{err}");
    }

    #[test]
    fn a_batch_is_judged_on_its_final_state() {
        // The API enabled without a config could not start a node, but a
        // later override in the batch disables it again.
        let mut config = test_config();
        let overrides: Vec<NodeConfigOverride> = [
            "enable-grpc-api=true",
            "grpc-api-config=null",
            "enable-grpc-api=false",
        ]
        .iter()
        .map(|input| input.parse().unwrap())
        .collect();
        apply_node_config_overrides(&overrides, &mut config).unwrap();
        assert!(!config.enable_grpc_api);
        assert!(config.grpc_api_config.is_none());

        // Only the final state is judged, so the section may be set before
        // the override that turns the feature on.
        let mut config = test_config();
        let overrides: Vec<NodeConfigOverride> = [
            "grpc-api-config={address: '0.0.0.0:60000'}",
            "enable-grpc-api=true",
        ]
        .iter()
        .map(|input| input.parse().unwrap())
        .collect();
        apply_node_config_overrides(&overrides, &mut config).unwrap();
        assert_eq!(
            config.grpc_api_config.unwrap().address,
            "0.0.0.0:60000".parse::<std::net::SocketAddr>().unwrap()
        );
    }

    #[test]
    fn a_rejected_batch_does_not_echo_the_override_values() {
        // The batch context names the overrides by scope and path: one
        // override's value must not be printed because another one was
        // rejected, since values carry credentials.
        let mut config = test_config();
        let overrides: Vec<NodeConfigOverride> = [
            "metrics.push-url=https://user:hunter2@example.com/push",
            "enable-grpc-api=true",
            "grpc-api-config=null",
        ]
        .iter()
        .map(|input| input.parse().unwrap())
        .collect();
        let err = format!(
            "{:#}",
            apply_node_config_overrides(&overrides, &mut config).unwrap_err()
        );
        assert!(!err.contains("hunter2"), "{err}");
        assert!(err.contains("no node could run with"), "{err}");
        assert!(
            err.contains("all:metrics.push-url, all:enable-grpc-api, all:grpc-api-config"),
            "{err}"
        );
    }

    #[test]
    fn only_field_names_survive_a_deserialization_error() {
        // The messages that name a field of the config schema and nothing
        // else are the only ones kept.
        for message in [
            "missing field `remote-fw-url`",
            "duplicate field `parameters`",
        ] {
            assert_eq!(safe_deserialization_message(message), message);
        }
        for message in [
            r#"invalid type: string "hunter2", expected a boolean"#,
            "invalid type: integer `91234567`, expected a boolean",
            "unknown protocol string: hunter2",
            "unknown variant `hunter2`, expected one of `NoOp`",
            "missing field `hunter2 unknown variant `x``",
        ] {
            assert_eq!(safe_deserialization_message(message), UNFIT_VALUE);
        }
    }

    #[test]
    fn an_unknown_variant_does_not_echo_the_override_value() {
        // A backtick in the value closes the span serde backticks it in,
        // so no part of the value may survive.
        let mut config = test_config();
        config.policy_config = Some(PolicyConfig::default());
        let config_override: NodeConfigOverride =
            "policy-config.spam-policy-type='hun`ter2'".parse().unwrap();
        let err = format!("{:#}", config_override.apply_to(&mut config).unwrap_err());
        assert!(!err.contains("hun"), "{err}");
        assert!(!err.contains("ter2"), "{err}");
        assert!(err.contains("all:policy-config.spam-policy-type"), "{err}");
    }

    #[test]
    fn a_missing_equals_sign_does_not_echo_the_input() {
        // Without the `=` the whole input reads as a value, credentials
        // and all.
        let err = "metrics.push-url:https://user:hunter2-token@example.com/push"
            .parse::<NodeConfigOverride>()
            .unwrap_err()
            .to_string();
        assert!(!err.contains("hunter2-token"), "{err}");
        assert_eq!(err, "expected `[scope:]<path>=<value>`");
    }

    #[test]
    fn an_unparsable_value_does_not_echo_itself() {
        let err = "metrics.push-url=*hunter2-token"
            .parse::<NodeConfigOverride>()
            .unwrap_err()
            .to_string();
        assert!(!err.contains("hunter2-token"), "{err}");
        assert!(err.contains("metrics.push-url"), "{err}");
    }

    #[test]
    fn section_and_dotted_spellings_are_equivalent() {
        // One pair on a plain section and one on a section a config carries
        // only while it deviates from its default.
        for (dotted_input, section_input) in [
            (
                "authority-store-pruning-config.num-epochs-to-retain=5",
                "authority-store-pruning-config={num-epochs-to-retain: 5}",
            ),
            (
                "grpc-api-config.address='0.0.0.0:60000'",
                "grpc-api-config={address: '0.0.0.0:60000'}",
            ),
        ] {
            let mut base = test_config();
            base.grpc_api_config = Some(GrpcApiConfig::default());
            let mut dotted = base.clone();
            dotted_input
                .parse::<NodeConfigOverride>()
                .unwrap()
                .apply_to(&mut dotted)
                .unwrap();

            let mut section = base;
            section_input
                .parse::<NodeConfigOverride>()
                .unwrap()
                .apply_to(&mut section)
                .unwrap();

            assert_eq!(
                debug_with_keys_loaded(&dotted),
                debug_with_keys_loaded(&section),
                "{dotted_input}"
            );
        }
    }

    #[test]
    fn field_paths_lists_one_path_per_leaf() {
        let config_override: NodeConfigOverride = "p2p-config={seed-peers: [], anemo-config: null}"
            .parse()
            .unwrap();
        assert_eq!(
            config_override.field_paths(),
            ["p2p-config.seed-peers", "p2p-config.anemo-config"]
        );

        let config_override: NodeConfigOverride = "enable-index-processing=false".parse().unwrap();
        assert_eq!(config_override.field_paths(), ["enable-index-processing"]);

        // An empty mapping still names the section itself.
        let config_override: NodeConfigOverride = "metrics={}".parse().unwrap();
        assert_eq!(config_override.field_paths(), ["metrics"]);

        let config_override: NodeConfigOverride =
            "p2p-config={state-sync: {interval-period-ms: 5}}"
                .parse()
                .unwrap();
        assert_eq!(
            config_override.field_paths(),
            ["p2p-config.state-sync.interval-period-ms"]
        );
    }

    #[test]
    fn winning_field_paths_keeps_the_last_override_per_field() {
        let set_ten: NodeConfigOverride = "metrics.push-interval-seconds=10".parse().unwrap();
        let set_twenty: NodeConfigOverride = "metrics.push-interval-seconds=20".parse().unwrap();
        let clear: NodeConfigOverride = "metrics=null".parse().unwrap();

        // A later override on the same field replaces the earlier entry.
        assert_eq!(
            winning_field_paths([&set_ten, &set_twenty]),
            ["metrics.push-interval-seconds"]
        );

        // A later override of a whole section drops the entries nested
        // inside it.
        assert_eq!(winning_field_paths([&set_ten, &clear]), ["metrics"]);

        // In the reverse order both steps are listed: the clear reset the
        // section and the later override set one field of it.
        assert_eq!(
            winning_field_paths([&clear, &set_ten]),
            ["metrics", "metrics.push-interval-seconds"]
        );
    }

    #[test]
    fn apply_keeps_consensus_config_presence() {
        // An override that creates the validator-only section on a fullnode
        // is rejected.
        let mut config = test_config();
        let create: NodeConfigOverride = "consensus-config.db-retention-epochs=2".parse().unwrap();
        let err = create.apply_to(&mut config).unwrap_err().to_string();
        assert!(err.contains("validator"), "unhelpful error: {err}");
        assert!(config.consensus_config.is_none());

        // An override that removes it from a validator is also rejected.
        let mut config = validator_test_config();
        let clear: NodeConfigOverride = "consensus-config=null".parse().unwrap();
        assert!(clear.apply_to(&mut config).is_err());
        assert!(config.consensus_config.is_some());

        // In a batch, the removing override is blamed, not an earlier one
        // that edits the same section.
        let mut config = validator_test_config();
        let overrides: Vec<NodeConfigOverride> = [
            "consensus-config.db-retention-epochs=2",
            "consensus-config=",
        ]
        .iter()
        .map(|input| input.parse().unwrap())
        .collect();
        let err = apply_node_config_overrides(&overrides, &mut config)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("`all:consensus-config` would remove `consensus-config`"),
            "{err}"
        );
        assert!(config.consensus_config.is_some());
    }
}
