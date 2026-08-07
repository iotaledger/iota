// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Generic key-value overrides applied to generated [`NodeConfig`]s.

use std::{fmt, str::FromStr};

use anyhow::{Context, anyhow, bail, ensure};
use iota_config::NodeConfig;
use serde_yaml::{Mapping, Value};

/// The key pair fields of a [`NodeConfig`]: they carry node identity, must
/// not be overridden, and consumers that render configs omit them.
pub const KEY_PAIR_FIELDS: &[&str] = &[
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
/// `scope` is `all` (default), `fullnode`, `validator` (all validators), or
/// `validator-<N>`; `path` is a dot-separated list of field names as they
/// appear in the config YAML; `value` is YAML — `null` (or an empty value)
/// clears a field, and clearing one that is already unset changes nothing;
/// a mapping merges with the section's current fields, an empty one
/// creating an unset section from its defaults; a list is replaced as a
/// whole.
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

/// The fields the given overrides set, each with the override that wins
/// it: later overrides shadow earlier entries for the same field and for
/// anything nested inside it.
pub fn winning_field_paths<'a>(
    overrides: impl IntoIterator<Item = &'a NodeConfigOverride>,
) -> Vec<(String, &'a NodeConfigOverride)> {
    let mut fields: Vec<(String, &'a NodeConfigOverride)> = Vec::new();
    for config_override in overrides {
        for field_path in config_override.field_paths() {
            fields.retain(|(path, _)| {
                *path != field_path && !path.starts_with(&format!("{field_path}."))
            });
            fields.push((field_path, config_override));
        }
    }
    fields
}

/// The sections the given overrides fill in from their default on a node
/// whose config has none: `grpc-api-config` when an override turns the
/// gRPC API on, `policy-config` when one edits that section or sets a
/// `firewall-config`. [`apply_node_config_overrides`] also weighs what the
/// config already carries, so it may fill in fewer of them.
pub fn sections_filled_in_from_defaults<'a>(
    overrides: impl IntoIterator<Item = &'a NodeConfigOverride>,
) -> Vec<&'static str> {
    let overrides: Vec<&NodeConfigOverride> = overrides.into_iter().collect();
    let last_on = |field: &str| {
        overrides
            .iter()
            .rev()
            .find(|config_override| config_override.path.first().is_some_and(|s| s == field))
    };
    let replaces_whole_section = |field: &str| {
        last_on(field).is_some_and(|config_override| {
            config_override.path == [field] && !config_override.value.is_mapping()
        })
    };
    let edits_policy_config = overrides.iter().any(|config_override| {
        config_override
            .path
            .first()
            .is_some_and(|s| s == "policy-config")
            && (config_override.path.len() > 1 || config_override.value.is_mapping())
    });
    let sets_firewall_config =
        last_on("firewall-config").is_some_and(|config_override| !config_override.value.is_null());
    let mut sections = Vec::new();
    if last_on("enable-grpc-api")
        .is_some_and(|config_override| config_override.value == Value::Bool(true))
        && !replaces_whole_section("grpc-api-config")
    {
        sections.push("grpc-api-config");
    }
    if (edits_policy_config || sets_firewall_config) && !replaces_whole_section("policy-config") {
        sections.push("policy-config");
    }
    sections
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
    /// of its value; an empty mapping names the section itself.
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

/// Fields that serde fills with a default when absent from the YAML.
///
/// The serialized config omits them when `None`, so an explicit `null`
/// keeps them unset across the round trip and an override cannot set them
/// behind the user's back; [`seed_absent_defaults`] inserts those nulls,
/// for this crate and for consumers that render a config. This only works
/// for `Option` fields under always-serialized sections; the list is
/// guarded by `apply_leaves_untouched_fields_unchanged`.
const FIELDS_DEFAULTED_WHEN_ABSENT: &[&[&str]] = &[
    &["grpc-api-config"],
    &["policy-config"],
    &[
        "authority-store-pruning-config",
        "periodic-compaction-threshold-days",
    ],
];

/// Apply `overrides` to `config` in the given order: later overrides win
/// per field, and fields no override mentions keep their current values.
/// Declaration order decides, not how specific a scope is: a later
/// `validator:` override beats an earlier `validator-0:` one.
///
/// An override that enables the gRPC API on a fullnode also gives the node
/// the default `grpc-api-config`. A node without a `policy-config` gets
/// the default one when an override edits that section or creates a
/// `firewall-config` where there was none. An override in the batch that
/// replaces the whole `grpc-api-config` or `policy-config`, `null`
/// included, wins over that default; one that sets fields inside either
/// section, by dotted path or as a whole-section mapping, edits the
/// default.
///
/// Unknown fields, type mismatches, and changes that would leave the node
/// unable to run are rejected; on error the config is left unchanged.
/// [`NodeConfig::validate`] judges the final state, so an override may
/// clear a field a later one restores, and a batch may repair a config
/// that was already invalid; creating or removing `consensus-config` is
/// rejected per override.
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
    // Judge the starting point without failing on it: the batch may repair
    // a config that could not run as it stands.
    let base_error = config.validate().err();

    // iota-node and iota-swarm decide whether a node runs as a validator
    // or a fullnode by whether it has a consensus config, so an override
    // must not create or remove that section.
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
    // override below; at localnet scale that cost is not worth avoiding.
    let mut merged = serde_yaml::to_value(&*config).context("failed to serialize node config")?;
    seed_absent_defaults(&mut merged);
    let firewall_was_unset = merged.get("firewall-config").is_none_or(Value::is_null);
    let grpc_api_config_was_unset = merged.get("grpc-api-config").is_none_or(Value::is_null);
    let policy_config_was_unset = merged.get("policy-config").is_none_or(Value::is_null);

    // Apply and check one override at a time, so a typo is caught even if
    // a later override overwrites the field, and the error names the
    // override at fault.
    let mut checked: Option<NodeConfig> = None;
    for config_override in &overrides {
        let mut cursor = &mut merged;
        for (i, segment) in config_override.path.iter().enumerate() {
            // An absent optional sub-config serializes as null; create the
            // mapping so a nested field of it can still be set.
            if cursor.is_null() {
                *cursor = Value::Mapping(Mapping::new());
            }
            let mapping = cursor.as_mapping_mut().ok_or_else(|| {
                anyhow!(
                    "`{}` in `{}` does not refer to a config section",
                    config_override.path[..i].join("."),
                    config_override.scoped_field_path()
                )
            })?;
            let key = Value::from(segment.as_str());
            if !mapping.contains_key(&key) {
                mapping.insert(key.clone(), Value::Null);
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

    // An override that replaces the whole section, `null` included, wins
    // over the default it would otherwise get; a whole-section mapping
    // merges with the section like a dotted path does, so it edits the
    // default instead. Later overrides win, so only the last override on
    // the section counts. Both rules read top-level fields only.
    let sets_field = |field: &str| {
        overrides
            .iter()
            .any(|config_override| config_override.path.first().is_some_and(|s| s == field))
    };
    let replaces_whole_section = |field: &str| {
        overrides
            .iter()
            .rev()
            .find(|config_override| config_override.path.first().is_some_and(|s| s == field))
            .is_some_and(|config_override| {
                config_override.path == [field] && !config_override.value.is_mapping()
            })
    };
    let edits_section = |field: &str| {
        overrides.iter().any(|config_override| {
            config_override.path.first().is_some_and(|s| s == field)
                && (config_override.path.len() > 1 || config_override.value.is_mapping())
        })
    };
    let is_fullnode = merged.get("consensus-config").is_none_or(Value::is_null);
    let default_grpc_api_config = is_fullnode
        && grpc_api_config_was_unset
        && merged.get("enable-grpc-api") == Some(&Value::Bool(true))
        && sets_field("enable-grpc-api")
        && !replaces_whole_section("grpc-api-config");
    let creates_firewall_config = firewall_was_unset
        && merged
            .get("firewall-config")
            .is_some_and(|value| !value.is_null());
    let default_policy_config = policy_config_was_unset
        && (edits_section("policy-config") || creates_firewall_config)
        && !replaces_whole_section("policy-config");
    // The defaults come from the serde default functions the two fields
    // name, so a materialized section is what deserializing a config
    // without the field would have produced.
    let mut materialized_sections = Vec::new();
    if default_grpc_api_config {
        let default = serde_yaml::to_value(
            iota_config::node::default_grpc_api_config()
                .expect("the serde default for `grpc-api-config` is a config, not `None`"),
        )
        .context("failed to serialize the default gRPC API config")?;
        materialize_default_section(&mut merged, "grpc-api-config", default);
        materialized_sections.push("grpc-api-config");
    }
    if default_policy_config {
        let default = serde_yaml::to_value(
            iota_config::node::default_traffic_controller_policy_config()
                .expect("the serde default for `policy-config` is a config, not `None`"),
        )
        .context("failed to serialize the default policy config")?;
        materialize_default_section(&mut merged, "policy-config", default);
        materialized_sections.push("policy-config");
    }

    // The last checked config already reflects the fully merged document;
    // only materializing a default above changes what deserialization
    // produces, and an override merged onto a default can conflict with
    // it, e.g. on two variants of the same enum. Materializing only adds
    // fields of the section's own default, so unlike the loop above this
    // needs no `serde_ignored` wrap to catch unknown keys.
    let mut new_config = if materialized_sections.is_empty() {
        checked.expect("the batch is non-empty")
    } else {
        // The context below keeps the error it is added to as its source,
        // and the binary prints the whole chain, so the raw serde error
        // must not be that source.
        let materialized = serde_yaml::from_value(merged).map_err(|err| {
            let message = err.to_string();
            anyhow!("{}", safe_deserialization_message(&message))
        });
        materialized.with_context(|| {
            let sections = materialized_sections
                .iter()
                .map(|section| format!("`{section}`"))
                .collect::<Vec<_>>()
                .join(" and ");
            // The default is not something the user wrote, so name the
            // overrides that were merged onto it, by scope and path.
            let culprits = overrides
                .iter()
                .filter(|config_override| {
                    config_override
                        .path
                        .first()
                        .is_some_and(|field| materialized_sections.contains(&field.as_str()))
                })
                .map(|config_override| config_override.scoped_field_path())
                .collect::<Vec<_>>();
            let mut context = format!(
                "the node config overrides produce an invalid config with the default {sections} \
                 filled in"
            );
            if !culprits.is_empty() {
                context.push_str(&format!(" (from {})", culprits.join(", ")));
            }
            context
        })?
    };

    // `supported_protocol_versions` is #[serde(skip)] on NodeConfig and
    // would be lost in the round trip; preserve it.
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
        match &base_error {
            // The batch may have repaired the config's original defect and
            // introduced another, so do not claim it is the same one.
            Some(base_error) => format!(
                "the node config was invalid before any overrides were applied \
                 ({base_error:#}); with {names} applied the config is invalid"
            ),
            None => format!("with {names} applied, no node could run with this config"),
        }
    })?;

    *config = new_config;
    Ok(())
}

/// Insert an explicit `null` at `field_path` in a serialized config when
/// the field is absent, leaving `root` unchanged when the section holding
/// the field is itself unset.
fn insert_null_if_absent(root: &mut Value, field_path: &[&str]) {
    let mut cursor = root;
    let (last, parents) = field_path.split_last().expect("field paths are not empty");
    for segment in parents {
        match cursor.get_mut(*segment) {
            Some(child) if child.is_mapping() => cursor = child,
            // The enclosing section is itself unset; its fields need no
            // explicit null.
            _ => return,
        }
    }
    let Some(mapping) = cursor.as_mapping_mut() else {
        return;
    };
    let key = Value::from(*last);
    if !mapping.contains_key(&key) {
        mapping.insert(key, Value::Null);
    }
}

/// Insert an explicit `null` in `root`, a serialized [`NodeConfig`], for
/// every field it omits that serde would otherwise fill with a default, so
/// the field is not read back as that default.
pub fn seed_absent_defaults(root: &mut Value) {
    for field_path in FIELDS_DEFAULTED_WHEN_ABSENT {
        insert_null_if_absent(root, field_path);
    }
}

/// Give `field` the default it would have without the seeded null: drop
/// the null, or, when an override set fields inside the section, merge
/// those onto `default` so the fields it did not set keep the default's
/// values rather than an empty section's.
fn materialize_default_section(root: &mut Value, field: &str, default: Value) {
    let mapping = root
        .as_mapping_mut()
        .expect("a serialized config is a mapping");
    let key = Value::from(field);
    match mapping.get_mut(&key) {
        Some(value) if value.is_mapping() => {
            let overridden_fields = std::mem::replace(value, default);
            merge_value(value, overridden_fields);
        }
        _ => {
            mapping.remove(&key);
        }
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

/// Reject mapping keys that are not strings anywhere in an override
/// value: serde reads a number as a field index, which names no field and
/// renders as a broken path in a list of overridden fields.
fn ensure_string_mapping_keys(value: &Value) -> anyhow::Result<()> {
    match value {
        Value::Mapping(mapping) => {
            for (key, value) in mapping {
                ensure!(
                    key.is_string(),
                    "mapping keys in an override value must be strings"
                );
                ensure_string_mapping_keys(value)?;
            }
        }
        Value::Sequence(sequence) => {
            for value in sequence {
                ensure_string_mapping_keys(value)?;
            }
        }
        _ => {}
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
/// custom deserializer can render anything at all, so only the two shapes
/// that name a field of the config schema and nothing else are kept.
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

/// Field names serde also accepts under a second spelling, with the one
/// the serialized config uses. Only an override path is normalized to that
/// spelling; an alias used as a key inside an override value collides with
/// the canonical key as a duplicate field when the config already carries
/// it.
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
    }

    #[test]
    fn clearing_a_section_the_node_does_not_have_changes_nothing() {
        // Including the validator-only `consensus-config`, which an
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
    fn an_invalid_base_config_is_not_blamed_on_the_overrides() {
        // A fullnode with the gRPC API enabled but no config cannot start,
        // and an unrelated override does not change that.
        let mut config = test_config();
        config.enable_grpc_api = true;
        config.grpc_api_config = None;
        let before = debug_with_keys_loaded(&config);

        let config_override: NodeConfigOverride = "enable-index-processing=false".parse().unwrap();
        let err = format!("{:#}", config_override.apply_to(&mut config).unwrap_err());
        assert!(err.contains("before any overrides"), "{err}");
        // The context carries the pre-existing defect and the overrides.
        assert!(err.contains("`grpc-api-config` is missing"), "{err}");
        assert!(
            err.contains("with `all:enable-index-processing` applied the config is invalid"),
            "{err}"
        );
        assert!(!err.contains("no node could run with"), "{err}");
        assert_eq!(debug_with_keys_loaded(&config), before);
    }

    #[test]
    fn an_override_may_repair_an_invalid_base_config() {
        // A firewall config without a policy config is inert, so the base
        // is invalid; an override that adds the policy, or that clears the
        // firewall, makes it valid.
        let mut base = test_config();
        base.firewall_config = Some(
            serde_yaml::from_str(
                "{remote-fw-url: 'http://127.0.0.1:65000', destination-port: 65000}",
            )
            .unwrap(),
        );
        assert!(base.validate().is_err());

        let mut config = base.clone();
        "policy-config={dry-run: false}"
            .parse::<NodeConfigOverride>()
            .unwrap()
            .apply_to(&mut config)
            .unwrap();
        assert!(config.policy_config.is_some());
        assert!(config.firewall_config.is_some());

        let mut config = base;
        "firewall-config="
            .parse::<NodeConfigOverride>()
            .unwrap()
            .apply_to(&mut config)
            .unwrap();
        assert!(config.firewall_config.is_none());
        assert!(config.policy_config.is_none());
    }

    #[test]
    fn a_field_name_alias_is_normalized_to_the_serialized_name() {
        // `consensus-config.parameters` also deserializes from
        // `starfish_parameters`; an edit under the alias must land on the
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

        // Merging it into a variant the config already spells out under the
        // serialized name is rejected: the two names sit side by side.
        let mut config = test_config();
        config.policy_config = Some(PolicyConfig::default_dos_protection_policy());
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

        // Clearing the config while the API is enabled is rejected, with
        // the startup invariant as the cause.
        let err = format!("{:#}", clear.apply_to(&mut config).unwrap_err());
        assert!(
            err.contains("`enable-grpc-api` is set but `grpc-api-config` is missing"),
            "{err}"
        );
        // The override is named by scope and path, without its value.
        assert!(
            err.contains("with `all:grpc-api-config` applied, no node could run with"),
            "{err}"
        );
        assert!(config.grpc_api_config.is_some());
    }

    #[test]
    fn enabling_the_grpc_api_on_a_validator_does_not_create_a_config() {
        // Validators do not expose the gRPC API, so enabling it neither
        // requires nor materializes a config there.
        let mut config = validator_test_config();
        let enable: NodeConfigOverride = "enable-grpc-api=true".parse().unwrap();
        enable.apply_to(&mut config).unwrap();
        assert!(config.enable_grpc_api);
        assert!(config.grpc_api_config.is_none());
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

        // An edit to an existing firewall config must not reset the policy
        // config to the serde default.
        let mut config = test_config();
        set.apply_to(&mut config).unwrap();
        let edit_policy: NodeConfigOverride = "policy-config.channel-capacity=42".parse().unwrap();
        edit_policy.apply_to(&mut config).unwrap();
        let edit: NodeConfigOverride = "firewall-config.destination-port=65001".parse().unwrap();
        edit.apply_to(&mut config).unwrap();
        assert_eq!(config.firewall_config.unwrap().destination_port, 65001);
        assert_eq!(config.policy_config.unwrap().channel_capacity, 42);

        // Clearing the policy config leaves the firewall inert, so it is
        // rejected while one is set.
        let mut config = test_config();
        set.apply_to(&mut config).unwrap();
        let clear_policy: NodeConfigOverride = "policy-config=null".parse().unwrap();
        let err = clear_policy.apply_to(&mut config).unwrap_err();
        let err = format!("{err:#}");
        assert!(err.contains("`firewall-config` is set"), "{err}");

        // The same holds within one batch: an explicit `policy-config`
        // clear next to a firewall create would leave the firewall inert.
        let mut config = test_config();
        let overrides: Vec<NodeConfigOverride> = [
            "firewall-config={remote-fw-url: 'http://127.0.0.1:65000', destination-port: 65000}",
            "policy-config=",
        ]
        .iter()
        .map(|input| input.parse().unwrap())
        .collect();
        let err = format!(
            "{:#}",
            apply_node_config_overrides(&overrides, &mut config).unwrap_err()
        );
        assert!(err.contains("`firewall-config` is set"), "{err}");
    }

    #[test]
    fn a_policy_config_edit_fills_in_the_default_policy_on_its_own() {
        // Without the default, the section would be read back from
        // `PolicyConfig`'s per-field serde defaults, so the same edit would
        // mean different unmentioned fields depending on the rest of the
        // batch.
        let base = test_config();
        for policy_input in [
            "policy-config.dry-run=false",
            "policy-config={dry-run: false}",
        ] {
            let mut config = base.clone();
            policy_input
                .parse::<NodeConfigOverride>()
                .unwrap()
                .apply_to(&mut config)
                .unwrap();
            let mut expected =
                serde_yaml::to_value(PolicyConfig::default_dos_protection_policy()).unwrap();
            expected
                .as_mapping_mut()
                .unwrap()
                .insert(Value::from("dry-run"), Value::Bool(false));
            assert_eq!(
                serde_yaml::to_value(config.policy_config.unwrap()).unwrap(),
                expected,
                "{policy_input}"
            );
        }

        // An empty mapping edits nothing, so the default stands as it is.
        let mut config = base;
        "policy-config={}"
            .parse::<NodeConfigOverride>()
            .unwrap()
            .apply_to(&mut config)
            .unwrap();
        assert_eq!(
            serde_yaml::to_value(config.policy_config.unwrap()).unwrap(),
            serde_yaml::to_value(PolicyConfig::default_dos_protection_policy()).unwrap()
        );
    }

    #[test]
    fn a_policy_config_clear_before_an_edit_still_fills_in_the_default() {
        // The clear is overwritten by the later edit, so it must not veto
        // the default.
        let mut config = test_config();
        let overrides: Vec<NodeConfigOverride> = ["policy-config=", "policy-config.dry-run=false"]
            .iter()
            .map(|input| input.parse().unwrap())
            .collect();
        apply_node_config_overrides(&overrides, &mut config).unwrap();
        let mut expected =
            serde_yaml::to_value(PolicyConfig::default_dos_protection_policy()).unwrap();
        expected
            .as_mapping_mut()
            .unwrap()
            .insert(Value::from("dry-run"), Value::Bool(false));
        assert_eq!(
            serde_yaml::to_value(config.policy_config.unwrap()).unwrap(),
            expected
        );
    }

    #[test]
    fn a_firewall_config_set_and_cleared_in_one_batch_creates_no_policy() {
        let mut config = test_config();
        let overrides: Vec<NodeConfigOverride> = [
            "firewall-config={remote-fw-url: 'http://127.0.0.1:65000', destination-port: 65000}",
            "firewall-config=",
        ]
        .iter()
        .map(|input| input.parse().unwrap())
        .collect();
        apply_node_config_overrides(&overrides, &mut config).unwrap();
        assert!(config.firewall_config.is_none());
        assert!(config.policy_config.is_none());
    }

    #[test]
    fn a_grpc_api_config_edit_alone_does_not_enable_the_api() {
        // Unlike `policy-config`, the gRPC default is filled in only when an
        // override enables the API, which is what the section depends on.
        let mut config = test_config();
        let config_override: NodeConfigOverride =
            "grpc-api-config.address='0.0.0.0:60000'".parse().unwrap();
        config_override.apply_to(&mut config).unwrap();
        assert!(!config.enable_grpc_api);
        assert_eq!(
            config.grpc_api_config.unwrap().address,
            "0.0.0.0:60000".parse::<std::net::SocketAddr>().unwrap()
        );
    }

    #[test]
    fn an_override_inside_the_materialized_policy_config_edits_it() {
        // A new firewall config materializes the default policy config,
        // and the override edits that default, in either spelling:
        // starting from an empty section instead would leave both policy
        // types `NoOp`.
        let firewall_input =
            "firewall-config={remote-fw-url: 'http://127.0.0.1:65000', destination-port: 65000}";
        let base = test_config();
        for policy_input in [
            "policy-config.dry-run=false",
            "policy-config={dry-run: false}",
        ] {
            let mut config = base.clone();
            let overrides: Vec<NodeConfigOverride> = [firewall_input, policy_input]
                .iter()
                .map(|input| input.parse().unwrap())
                .collect();
            apply_node_config_overrides(&overrides, &mut config).unwrap();
            let mut expected =
                serde_yaml::to_value(PolicyConfig::default_dos_protection_policy()).unwrap();
            expected
                .as_mapping_mut()
                .unwrap()
                .insert(Value::from("dry-run"), Value::Bool(false));
            assert_eq!(
                serde_yaml::to_value(config.policy_config.unwrap()).unwrap(),
                expected,
                "{policy_input}"
            );
        }
    }

    #[test]
    fn an_override_inside_the_materialized_grpc_api_config_edits_it() {
        // Enabling the gRPC API materializes its default config, and the
        // override edits that default, in either spelling.
        let base = test_config();
        for grpc_input in [
            "grpc-api-config.address='0.0.0.0:60000'",
            "grpc-api-config={address: '0.0.0.0:60000'}",
        ] {
            let mut config = base.clone();
            let overrides: Vec<NodeConfigOverride> = ["enable-grpc-api=true", grpc_input]
                .iter()
                .map(|input| input.parse().unwrap())
                .collect();
            apply_node_config_overrides(&overrides, &mut config).unwrap();
            let mut expected = serde_yaml::to_value(GrpcApiConfig::default()).unwrap();
            expected
                .as_mapping_mut()
                .unwrap()
                .insert(Value::from("address"), Value::from("0.0.0.0:60000"));
            assert_eq!(
                serde_yaml::to_value(config.grpc_api_config.unwrap()).unwrap(),
                expected,
                "{grpc_input}"
            );
        }
    }

    #[test]
    fn sections_filled_in_from_defaults_agrees_with_apply() {
        // The list is read off the batch alone, so it has to match what the
        // engine fills in on a config that carries neither section.
        const FIREWALL: &str =
            "firewall-config={remote-fw-url: 'http://127.0.0.1:65000', destination-port: 65000}";
        for inputs in [
            vec!["enable-grpc-api=true"],
            vec!["enable-grpc-api=false"],
            vec!["policy-config.dry-run=false"],
            vec!["policy-config="],
            vec![FIREWALL],
            vec![FIREWALL, "firewall-config="],
        ] {
            let mut config = test_config();
            assert!(config.grpc_api_config.is_none());
            assert!(config.policy_config.is_none());
            let overrides: Vec<NodeConfigOverride> =
                inputs.iter().map(|input| input.parse().unwrap()).collect();
            apply_node_config_overrides(&overrides, &mut config).unwrap();

            let sections = sections_filled_in_from_defaults(&overrides);
            assert_eq!(
                sections.contains(&"grpc-api-config"),
                config.grpc_api_config.is_some(),
                "{inputs:?}"
            );
            assert_eq!(
                sections.contains(&"policy-config"),
                config.policy_config.is_some(),
                "{inputs:?}"
            );
        }
    }

    #[test]
    fn an_override_that_does_not_merge_into_a_materialized_default_is_rejected() {
        // Merging a mapping into the default keeps the default's fields
        // the override does not mention, which for an enum means two
        // variants at once.
        let mut config = test_config();
        let before = debug_with_keys_loaded(&config);
        let overrides: Vec<NodeConfigOverride> = [
            "firewall-config={remote-fw-url: 'http://127.0.0.1:65000', destination-port: 65000}",
            "policy-config.spam-policy-type={TestNConnIP: 5}",
        ]
        .iter()
        .map(|input| input.parse().unwrap())
        .collect();
        let err = apply_node_config_overrides(&overrides, &mut config)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains(
                "the node config overrides produce an invalid config with the default \
                 `policy-config` filled in"
            ),
            "{err}"
        );
        // The default is not something the user wrote, so the override that
        // was merged onto it is named.
        assert!(
            err.contains("(from all:policy-config.spam-policy-type)"),
            "{err}"
        );
        assert_eq!(debug_with_keys_loaded(&config), before);
    }

    #[test]
    fn parse_rejects_non_string_mapping_keys() {
        // Serde reads an integer key as a field index, which would render
        // as a broken path in a list of overridden fields.
        for input in ["metrics={0: 5}", "p2p-config={seed-peers: [{0: 1}]}"] {
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
        // Including `periodic-compaction-threshold-days`, whose `None` does
        // not survive the round trip on its own (serde default `Some(1)`).
        assert_eq!(
            config
                .authority_store_pruning_config
                .periodic_compaction_threshold_days,
            None
        );

        // A mentioned field is set, even one from
        // `FIELDS_DEFAULTED_WHEN_ABSENT`.
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

        // Recreating the section from a bare mapping would silently reset
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

        // Nesting under a field an earlier override set to a scalar blames
        // the nesting override, not the earlier valid one.
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

        // An override that sets `grpc-api-config` itself wins over the
        // materialized default, even when it comes first.
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
    fn a_type_mismatch_does_not_echo_the_override_value() {
        // The value of the override at fault carries credentials just as
        // often as the values of the others, and serde renders a number
        // without the quotes it puts around a string.
        let mut config = test_config();
        let config_override: NodeConfigOverride =
            "enable-index-processing=91234567".parse().unwrap();
        let err = format!("{:#}", config_override.apply_to(&mut config).unwrap_err());
        assert!(!err.contains("91234567"), "{err}");
        assert!(err.contains("all:enable-index-processing"), "{err}");
    }

    #[test]
    fn a_custom_deserializer_error_does_not_echo_the_override_value() {
        // A Multiaddr reports an unknown protocol with the string bare,
        // outside any quoting serde's own messages use.
        let mut config = test_config();
        let config_override: NodeConfigOverride =
            "network-address=/hunter2-token/tcp/1".parse().unwrap();
        let err = format!("{:#}", config_override.apply_to(&mut config).unwrap_err());
        assert!(!err.contains("hunter2-token"), "{err}");
        assert!(err.contains("all:network-address"), "{err}");
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
        let config_override: NodeConfigOverride =
            "policy-config.spam-policy-type='hun`ter2'".parse().unwrap();
        let err = format!("{:#}", config_override.apply_to(&mut config).unwrap_err());
        assert!(!err.contains("hun"), "{err}");
        assert!(!err.contains("ter2"), "{err}");
        assert!(err.contains("all:policy-config.spam-policy-type"), "{err}");
    }

    #[test]
    fn a_materialization_conflict_does_not_echo_the_override_value() {
        // The error is built after the batch is merged onto a default, so
        // it must be sanitized like the per-override ones, source chain
        // included: the binary prints errors with `{err:?}`.
        let mut config = test_config();
        let config_override: NodeConfigOverride =
            "policy-config={spam-policy-type: {TestNConnIP: 5}, allow-list: ['hunter2-token']}"
                .parse()
                .unwrap();
        let err = format!("{:?}", config_override.apply_to(&mut config).unwrap_err());
        assert!(!err.contains("hunter2-token"), "{err}");
        assert!(err.contains("`policy-config` filled in"), "{err}");
        assert!(err.contains("(from all:policy-config)"), "{err}");
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
    fn a_value_mapping_may_replace_a_scalar_enum_variant() {
        // `spam-policy-type` serializes as a bare string for unit variants
        // and as a mapping for struct variants; switching variants must
        // work.
        let mut config = test_config();
        let overrides: Vec<NodeConfigOverride> = [
            "policy-config={spam-policy-type: NoOp}",
            "policy-config.spam-policy-type={freq-threshold: {client-threshold: 5}}",
        ]
        .iter()
        .map(|input| input.parse().unwrap())
        .collect();
        apply_node_config_overrides(&overrides, &mut config).unwrap();
        let policy_config = format!("{:?}", config.policy_config.unwrap());
        assert!(policy_config.contains("FreqThreshold"), "{policy_config}");
    }

    #[test]
    fn section_and_dotted_spellings_are_equivalent() {
        // One pair on a plain section and one on a field from
        // `FIELDS_DEFAULTED_WHEN_ABSENT`, whose two spellings take
        // different routes through the explicit-null handling.
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
            let base = test_config();
            let mut dotted = base.clone();
            dotted_input
                .parse::<NodeConfigOverride>()
                .unwrap()
                .apply_to(&mut dotted)
                .unwrap();

            let mut section = base.clone();
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
        let config_override: NodeConfigOverride =
            "p2p-config={external-address: null, seed-peers: []}"
                .parse()
                .unwrap();
        assert_eq!(
            config_override.field_paths(),
            ["p2p-config.external-address", "p2p-config.seed-peers"]
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
        fn winners(overrides: [&NodeConfigOverride; 2]) -> Vec<(String, String)> {
            winning_field_paths(overrides)
                .into_iter()
                .map(|(field_path, config_override)| (field_path, config_override.to_string()))
                .collect()
        }
        let set_ten: NodeConfigOverride = "metrics.push-interval-seconds=10".parse().unwrap();
        let set_twenty: NodeConfigOverride = "metrics.push-interval-seconds=20".parse().unwrap();
        let clear: NodeConfigOverride = "metrics=null".parse().unwrap();

        // A later override on the same field replaces the earlier entry...
        assert_eq!(
            winners([&set_ten, &set_twenty]),
            [(
                "metrics.push-interval-seconds".to_owned(),
                "all:metrics.push-interval-seconds=20".to_owned(),
            )]
        );

        // ...and a later override of a whole section drops the entries
        // nested inside it.
        assert_eq!(
            winners([&set_ten, &clear]),
            [("metrics".to_owned(), "all:metrics=~".to_owned())]
        );

        // In the reverse order both steps are listed: the clear reset the
        // section and the later override set one field of it.
        assert_eq!(
            winners([&clear, &set_ten]),
            [
                ("metrics".to_owned(), "all:metrics=~".to_owned()),
                (
                    "metrics.push-interval-seconds".to_owned(),
                    "all:metrics.push-interval-seconds=10".to_owned(),
                ),
            ]
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
            "validator:enable-soft-locking=false".parse().unwrap();
        assert_eq!(
            config_override.to_string(),
            "validator:enable-soft-locking=false"
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
