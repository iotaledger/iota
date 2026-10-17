// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_move_types::iota_framework::object::{ID, UID};
use iota_sdk_types::StructTag;
use move_core_types::{
    account_address::AccountAddress,
    annotated_value::{MoveFieldLayout, MoveStructLayout, MoveTypeLayout},
    ident_str,
    identifier::IdentStr,
};

use crate::{IOTA_FRAMEWORK_ADDRESS, iota_sdk_types_conversions::struct_tag_sdk_to_core};

pub const RESOLVED_IOTA_ID: (&AccountAddress, &IdentStr, &IdentStr) = (
    &IOTA_FRAMEWORK_ADDRESS,
    ident_str!("object"),
    ident_str!("ID"),
);

mod id_ext {
    pub trait Sealed {}
    impl Sealed for super::ID {}
    impl Sealed for super::UID {}
}

pub trait IDExt: Sized + id_ext::Sealed {
    fn layout() -> MoveStructLayout;
}

impl IDExt for ID {
    fn layout() -> MoveStructLayout {
        MoveStructLayout {
            type_: struct_tag_sdk_to_core(&StructTag::new_id()),
            fields: vec![MoveFieldLayout::new(
                ident_str!("bytes").to_owned(),
                MoveTypeLayout::Address,
            )],
        }
    }
}

impl IDExt for UID {
    fn layout() -> MoveStructLayout {
        MoveStructLayout {
            type_: struct_tag_sdk_to_core(&StructTag::new_uid()),
            fields: vec![MoveFieldLayout::new(
                ident_str!("id").to_owned(),
                MoveTypeLayout::Struct(Box::new(ID::layout())),
            )],
        }
    }
}
