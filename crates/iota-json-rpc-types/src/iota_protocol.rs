// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use iota_protocol_config::{ProtocolConfig, ProtocolConfigValue, ProtocolVersion};
use iota_types::move_package::ProtocolBuildConfig;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};

use crate::iota_primitives::ProtocolVersion as ProtocolVersionSchema;

/// Feature-flag key mirroring
/// [`ProtocolConfig::package_metadata_with_dynamic_module_metadata`]; the flag
/// is exposed in [`ProtocolConfigResponse::feature_flags`] under its field
/// name.
const PACKAGE_METADATA_WITH_DYNAMIC_MODULE_METADATA: &str =
    "package_metadata_with_dynamic_module_metadata";

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase", rename = "ProtocolConfigValue")]
pub enum IotaProtocolConfigValue {
    U16(
        #[schemars(with = "String")]
        #[serde_as(as = "DisplayFromStr")]
        u16,
    ),
    U32(
        #[schemars(with = "String")]
        #[serde_as(as = "DisplayFromStr")]
        u32,
    ),
    U64(
        #[schemars(with = "String")]
        #[serde_as(as = "DisplayFromStr")]
        u64,
    ),
    F64(
        #[schemars(with = "String")]
        #[serde_as(as = "DisplayFromStr")]
        f64,
    ),
    Bool(
        #[schemars(with = "String")]
        #[serde_as(as = "DisplayFromStr")]
        bool,
    ),
}

impl From<ProtocolConfigValue> for IotaProtocolConfigValue {
    fn from(value: ProtocolConfigValue) -> Self {
        match value {
            ProtocolConfigValue::u16(y) => IotaProtocolConfigValue::U16(y),
            ProtocolConfigValue::u32(y) => IotaProtocolConfigValue::U32(y),
            ProtocolConfigValue::u64(x) => IotaProtocolConfigValue::U64(x),
            ProtocolConfigValue::bool(z) => IotaProtocolConfigValue::Bool(z),
        }
    }
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase", rename = "ProtocolConfig")]
pub struct ProtocolConfigResponse {
    #[schemars(with = "ProtocolVersionSchema")]
    #[serde_as(as = "ProtocolVersionSchema")]
    pub min_supported_protocol_version: ProtocolVersion,
    #[schemars(with = "ProtocolVersionSchema")]
    #[serde_as(as = "ProtocolVersionSchema")]
    pub max_supported_protocol_version: ProtocolVersion,
    #[schemars(with = "ProtocolVersionSchema")]
    #[serde_as(as = "ProtocolVersionSchema")]
    pub protocol_version: ProtocolVersion,
    pub feature_flags: BTreeMap<String, bool>,
    pub attributes: BTreeMap<String, Option<IotaProtocolConfigValue>>,
}

impl From<&ProtocolConfigResponse> for ProtocolBuildConfig {
    fn from(response: &ProtocolConfigResponse) -> Self {
        Self {
            allow_view_function: response
                .feature_flags
                .get(PACKAGE_METADATA_WITH_DYNAMIC_MODULE_METADATA)
                .copied()
                .unwrap_or(false),
        }
    }
}

impl From<ProtocolConfig> for ProtocolConfigResponse {
    fn from(config: ProtocolConfig) -> Self {
        ProtocolConfigResponse {
            protocol_version: config.version,
            attributes: config
                .attr_map()
                .into_iter()
                .map(|(k, v)| (k, v.map(IotaProtocolConfigValue::from)))
                .collect(),
            min_supported_protocol_version: ProtocolVersion::MIN,
            max_supported_protocol_version: ProtocolVersion::MAX,
            feature_flags: config.feature_map(),
        }
    }
}
