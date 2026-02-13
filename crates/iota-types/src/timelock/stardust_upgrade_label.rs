// Copyright (c) 2024 IOTA Stiftung
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::base_types::{Identifier, IotaAddress, StructTag};

pub const STARDUST_UPGRADE_MODULE_NAME: Identifier =
    Identifier::from_static("stardust_upgrade_label");
pub const STARDUST_UPGRADE_STRUCT_NAME: Identifier =
    Identifier::from_static("STARDUST_UPGRADE_LABEL");

pub const STARDUST_UPGRADE_LABEL_VALUE: &str = "000000000000000000000000000000000000000000000000000000000000107a::stardust_upgrade_label::STARDUST_UPGRADE_LABEL";

/// Get the stardust upgrade label `type`.
pub fn stardust_upgrade_label_type() -> StructTag {
    StructTag::new(
        IotaAddress::STARDUST,
        STARDUST_UPGRADE_MODULE_NAME,
        STARDUST_UPGRADE_STRUCT_NAME,
        vec![],
    )
}

/// Is this other StructTag representing a stardust upgrade label?
pub fn is_stardust_upgrade(other: &StructTag) -> bool {
    other.address() == IotaAddress::STARDUST
        && other.module() == &STARDUST_UPGRADE_MODULE_NAME
        && other.name() == &STARDUST_UPGRADE_STRUCT_NAME
}
