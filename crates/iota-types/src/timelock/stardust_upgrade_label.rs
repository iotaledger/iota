// Copyright (c) 2024 IOTA Stiftung
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use move_core_types::{ident_str, identifier::IdentStr, language_storage::StructTag};

use crate::STARDUST_ADDRESS;

pub const STARDUST_UPGRADE_MODULE_NAME: &IdentStr = ident_str!("stardust_upgrade_label");
pub const STARDUST_UPGRADE_STRUCT_NAME: &IdentStr = ident_str!("STARDUST_UPGRADE_LABEL");

pub const STARDUST_UPGRADE_LABEL_VALUE: &str = "000000000000000000000000000000000000000000000000000000000000107a::stardust_upgrade_label::STARDUST_UPGRADE_LABEL";

/// Get the stardust upgrade label `type`.
pub fn stardust_upgrade_label_type() -> StructTag {
    StructTag {
        address: STARDUST_ADDRESS,
        module: STARDUST_UPGRADE_MODULE_NAME.to_owned(),
        name: STARDUST_UPGRADE_STRUCT_NAME.to_owned(),
        type_params: vec![],
    }
}

/// Is this other StructTag representing a stardust upgrade label?
pub fn is_stardust_upgrade(other: &StructTag) -> bool {
    other.address == STARDUST_ADDRESS
        && other.module.as_ident_str() == STARDUST_UPGRADE_MODULE_NAME
        && other.name.as_ident_str() == STARDUST_UPGRADE_STRUCT_NAME
}
