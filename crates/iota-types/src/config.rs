// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::{IdentifierRef, StructTag, TypeTag};
use move_core_types::{account_address::AccountAddress, ident_str, identifier::IdentStr};
use serde::{Deserialize, Serialize};

use crate::{IOTA_FRAMEWORK_ADDRESS, MoveTypeTagTrait, base_types::EpochId, id::UID};

pub const CONFIG_MODULE_NAME: &IdentifierRef = IdentifierRef::const_new("config");
pub const CONFIG_STRUCT_NAME: &IdentifierRef = IdentifierRef::const_new("Config");
pub const SETTING_STRUCT_NAME: &IdentifierRef = IdentifierRef::const_new("Setting");
pub const SETTING_DATA_STRUCT_NAME: &IdentifierRef = IdentifierRef::const_new("SettingData");
pub const RESOLVED_IOTA_CONFIG: (&AccountAddress, &IdentStr, &IdentStr) = (
    &IOTA_FRAMEWORK_ADDRESS,
    ident_str!(CONFIG_MODULE_NAME.as_str()),
    ident_str!(CONFIG_STRUCT_NAME.as_str()),
);

/// Rust representation of the Move type 0x2::config::Config.
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub id: UID,
}

/// Rust representation of the Move type 0x2::config::Setting.
#[derive(Debug, Serialize, Deserialize)]
pub struct Setting<V> {
    pub data: Option<SettingData<V>>,
}

/// Rust representation of the Move type 0x2::config::SettingData.
#[derive(Debug, Serialize, Deserialize)]
pub struct SettingData<V> {
    pub newer_value_epoch: u64,
    pub newer_value: Option<V>,
    pub older_value_opt: Option<V>,
}

impl Config {
    pub fn type_() -> StructTag {
        StructTag::new_config()
    }
}

pub fn setting_type(value_tag: TypeTag) -> StructTag {
    StructTag::new_config_setting(value_tag)
}

impl MoveTypeTagTrait for Config {
    fn get_type_tag() -> TypeTag {
        TypeTag::Struct(Box::new(Config::type_()))
    }
}

impl<V: MoveTypeTagTrait> MoveTypeTagTrait for Setting<V> {
    fn get_type_tag() -> TypeTag {
        TypeTag::Struct(Box::new(setting_type(V::get_type_tag())))
    }
}

pub fn is_setting(tag: &StructTag) -> bool {
    tag.is_config_setting()
}

impl<V> Setting<V> {
    /// Calls `SettingData::read_value` on the setting's data.
    /// The `data` should never be `None`, but for safety, this method returns
    /// `None` if it is.
    pub fn read_value(&self, cur_epoch: Option<EpochId>) -> Option<&V> {
        self.data.as_ref()?.read_value(cur_epoch)
    }
}

impl<V> SettingData<V> {
    /// Reads the value of the setting, giving `newer_value` if the current
    /// epoch is greater than `newer_value_epoch`, and `older_value_opt`
    /// otherwise. If `cur_epoch` is `None`, the `newer_value` is always
    /// returned.
    pub fn read_value(&self, cur_epoch: Option<EpochId>) -> Option<&V> {
        let use_newer_value = match cur_epoch {
            Some(cur_epoch) => cur_epoch > self.newer_value_epoch,
            None => true,
        };
        if use_newer_value {
            self.newer_value.as_ref()
        } else {
            self.older_value_opt.as_ref()
        }
    }
}
