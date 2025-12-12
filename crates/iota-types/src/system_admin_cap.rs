// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_2::types::IdentifierRef;

pub const IOTA_SYSTEM_ADMIN_CAP_MODULE_NAME: &IdentifierRef =
    IdentifierRef::const_new("system_admin_cap");
pub const IOTA_SYSTEM_ADMIN_CAP_STRUCT_NAME: &IdentifierRef =
    IdentifierRef::const_new("IotaSystemAdminCap");

pub use checked::*;

#[iota_macros::with_checked_arithmetic]
mod checked {
    use iota_sdk_2::types::{Address, StructTag};
    use serde::{Deserialize, Serialize};

    use super::*;

    /// Rust version of the IotaSystemAdminCap type.
    #[derive(Debug, Default, Serialize, Deserialize, Clone, Eq, PartialEq)]
    pub struct IotaSystemAdminCap {
        // This field is required to make a Rust struct compatible with an empty Move one.
        // An empty Move struct contains a 1-byte dummy bool field because empty fields are not
        // allowed in the bytecode.
        dummy_field: bool,
    }

    impl IotaSystemAdminCap {
        pub fn type_() -> StructTag {
            StructTag {
                address: Address::FRAMEWORK,
                module: IOTA_SYSTEM_ADMIN_CAP_MODULE_NAME.to_owned(),
                name: IOTA_SYSTEM_ADMIN_CAP_STRUCT_NAME.to_owned(),
                type_params: Vec::new(),
            }
        }
    }
}
