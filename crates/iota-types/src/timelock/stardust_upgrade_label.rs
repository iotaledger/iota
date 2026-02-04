// Copyright (c) 2024 IOTA Stiftung
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::{IdentifierRef, StructTag};

use crate::STARDUST_ADDRESS;

pub const STARDUST_UPGRADE_MODULE_NAME: &IdentifierRef =
    IdentifierRef::const_new("stardust_upgrade_label");
pub const STARDUST_UPGRADE_STRUCT_NAME: &IdentifierRef =
    IdentifierRef::const_new("STARDUST_UPGRADE_LABEL");

pub const STARDUST_UPGRADE_LABEL_VALUE: &str = "000000000000000000000000000000000000000000000000000000000000107a::stardust_upgrade_label::STARDUST_UPGRADE_LABEL";

/// Get the stardust upgrade label `type`.
pub fn stardust_upgrade_label_type() -> StructTag {
    StructTag::new(
        STARDUST_ADDRESS,
        STARDUST_UPGRADE_MODULE_NAME,
        STARDUST_UPGRADE_STRUCT_NAME,
        vec![],
    )
}

/// Is this other StructTag representing a stardust upgrade label?
pub fn is_stardust_upgrade(other: &StructTag) -> bool {
    other.address() == STARDUST_ADDRESS
        && other.module() == STARDUST_UPGRADE_MODULE_NAME
        && other.name() == STARDUST_UPGRADE_STRUCT_NAME
}
