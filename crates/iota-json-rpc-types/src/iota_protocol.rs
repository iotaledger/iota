// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use iota_protocol_config::{ProtocolConfig, ProtocolConfigValue, ProtocolVersion};
use iota_types::iota_serde::{AsProtocolVersion, BigInt, Readable};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase", rename = "ProtocolConfigValue")]
pub enum IotaProtocolConfigValue {
    U16(
        #[schemars(with = "BigInt<u16>")]
        #[serde_as(as = "BigInt<u16>")]
        u16,
    ),
    U32(
        #[schemars(with = "BigInt<u32>")]
        #[serde_as(as = "BigInt<u32>")]
        u32,
    ),
    U64(
        #[schemars(with = "BigInt<u64>")]
        #[serde_as(as = "BigInt<u64>")]
        u64,
    ),
    F64(
        #[schemars(with = "String")]
        #[serde_as(as = "DisplayFromStr")]
        f64,
    ),
}

impl IotaProtocolConfigValue {
    /// Checks whether the config value is a u16.
    pub fn is_u16(&self) -> bool {
        matches!(self, Self::U16(_))
    }

    /// Gets the config value as a u16.
    /// PANIC: Do not call on a non-u16 value.
    pub fn as_u16(&self) -> u16 {
        if let Self::U16(v) = self {
            *v
        } else {
            panic!("as_u16 called on a non-u16 {self:?}");
        }
    }

    /// Gets the config value as a u16, if it is one.
    pub fn as_u16_opt(&self) -> Option<u16> {
        if let Self::U16(v) = self {
            Some(*v)
        } else {
            None
        }
    }

    /// Checks whether the config value is a u32.
    pub fn is_u32(&self) -> bool {
        matches!(self, Self::U32(_))
    }

    /// Gets the config value as a u32.
    /// PANIC: Do not call on a non-u32 value.
    pub fn as_u32(&self) -> u32 {
        if let Self::U32(v) = self {
            *v
        } else {
            panic!("as_u32 called on a non-u32 {self:?}");
        }
    }

    /// Gets the config value as a u32, if it is one.
    pub fn as_u32_opt(&self) -> Option<u32> {
        if let Self::U32(v) = self {
            Some(*v)
        } else {
            None
        }
    }

    /// Checks whether the config value is a u64.
    pub fn is_u64(&self) -> bool {
        matches!(self, Self::U64(_))
    }

    /// Gets the config value as a u64.
    /// PANIC: Do not call on a non-u64 value.
    pub fn as_u64(&self) -> u64 {
        if let Self::U64(v) = self {
            *v
        } else {
            panic!("as_u64 called on a non-u64 {self:?}");
        }
    }

    /// Gets the config value as a u64, if it is one.
    pub fn as_u64_opt(&self) -> Option<u64> {
        if let Self::U64(v) = self {
            Some(*v)
        } else {
            None
        }
    }

    /// Checks whether the config value is a f64.
    pub fn is_f64(&self) -> bool {
        matches!(self, Self::F64(_))
    }

    /// Gets the config value as a f64.
    /// PANIC: Do not call on a non-f64 value.
    pub fn as_f64(&self) -> f64 {
        if let Self::F64(v) = self {
            *v
        } else {
            panic!("as_f64 called on a non-f64 {self:?}");
        }
    }

    /// Gets the config value as a f64, if it is one.
    pub fn as_f64_opt(&self) -> Option<f64> {
        if let Self::F64(v) = self {
            Some(*v)
        } else {
            None
        }
    }
}

impl From<ProtocolConfigValue> for IotaProtocolConfigValue {
    fn from(value: ProtocolConfigValue) -> Self {
        match value {
            ProtocolConfigValue::u16(y) => IotaProtocolConfigValue::U16(y),
            ProtocolConfigValue::u32(y) => IotaProtocolConfigValue::U32(y),
            ProtocolConfigValue::u64(x) => IotaProtocolConfigValue::U64(x),
            ProtocolConfigValue::f64(z) => IotaProtocolConfigValue::F64(z),
        }
    }
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase", rename = "ProtocolConfig")]
pub struct ProtocolConfigResponse {
    #[schemars(with = "AsProtocolVersion")]
    #[serde_as(as = "Readable<AsProtocolVersion, _>")]
    pub min_supported_protocol_version: ProtocolVersion,
    #[schemars(with = "AsProtocolVersion")]
    #[serde_as(as = "Readable<AsProtocolVersion, _>")]
    pub max_supported_protocol_version: ProtocolVersion,
    #[schemars(with = "AsProtocolVersion")]
    #[serde_as(as = "Readable<AsProtocolVersion, _>")]
    pub protocol_version: ProtocolVersion,
    pub feature_flags: BTreeMap<String, bool>,
    pub attributes: BTreeMap<String, IotaProtocolConfigValue>,
}

impl From<ProtocolConfig> for ProtocolConfigResponse {
    fn from(config: ProtocolConfig) -> Self {
        ProtocolConfigResponse {
            protocol_version: config.version,
            attributes: config
                .attr_map()
                .into_iter()
                .filter_map(|(k, v)| v.map(|v| (k, IotaProtocolConfigValue::from(v))))
                .collect(),
            min_supported_protocol_version: ProtocolVersion::MIN,
            max_supported_protocol_version: ProtocolVersion::MAX,
            feature_flags: config.feature_map(),
        }
    }
}
