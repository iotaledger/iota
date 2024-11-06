// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use move_core_types::{ident_str, identifier::IdentStr};

pub const IOTA_SYSTEM_ADMIN_CAP_MODULE_NAME: &IdentStr = ident_str!("system_admin_cap");
pub const IOTA_SYSTEM_ADMIN_CAP_STRUCT_NAME: &IdentStr = ident_str!("IotaSystemAdminCap");

pub use checked::*;

#[iota_macros::with_checked_arithmetic]
mod checked {
    use move_core_types::language_storage::StructTag;
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::IOTA_FRAMEWORK_ADDRESS;

    /// Rust version of the IotaSystemAdminCap type.
    #[derive(Debug, Default, Serialize, Deserialize, Clone, Eq, PartialEq)]
    pub struct IotaSystemAdminCap {
        // This field is required to make a Rust struct compatible with an empty Move one.
        // An empty Move struct contains a 1 byte dummy field because empty fields are not allowed
        // in the bytecode.
        dummy_field: u8,
    }

    impl IotaSystemAdminCap {
        pub fn type_() -> StructTag {
            StructTag {
                address: IOTA_FRAMEWORK_ADDRESS,
                module: IOTA_SYSTEM_ADMIN_CAP_MODULE_NAME.to_owned(),
                name: IOTA_SYSTEM_ADMIN_CAP_STRUCT_NAME.to_owned(),
                type_params: Vec::new(),
            }
        }
    }
}
