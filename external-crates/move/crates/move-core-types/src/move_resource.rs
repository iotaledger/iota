// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use serde::de::DeserializeOwned;

use crate::{
    account_address::AccountAddress,
    identifier::{IdentStr, Identifier},
    language_storage::{StructTag, TypeTag, access_vector},
};

pub trait MoveStructType {
    const ADDRESS: AccountAddress = crate::language_storage::CORE_CODE_ADDRESS;
    const MODULE_NAME: &'static IdentStr;
    const STRUCT_NAME: &'static IdentStr;

    fn module_identifier() -> Identifier {
        Self::MODULE_NAME.to_owned()
    }

    fn struct_identifier() -> Identifier {
        Self::STRUCT_NAME.to_owned()
    }

    fn type_params() -> Vec<TypeTag> {
        vec![]
    }

    fn struct_tag() -> StructTag {
        StructTag::new(
            Self::ADDRESS,
            Self::struct_identifier(),
            Self::module_identifier(),
            Self::type_params(),
        )
    }
}

pub trait MoveResource: MoveStructType + DeserializeOwned {
    fn resource_path() -> Vec<u8> {
        access_vector(&Self::struct_tag())
    }
}
