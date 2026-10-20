// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_move_types::iota_framework::object::{ID, UID};
use iota_sdk_types::StructTag;
use move_core_types::{
    annotated_value::{MoveFieldLayout, MoveStructLayout, MoveTypeLayout},
    ident_str,
};

use crate::iota_sdk_types_conversions::struct_tag_sdk_to_core;

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::ID {}
    impl Sealed for super::UID {}
}

pub trait MoveLayout: Sized + sealed::Sealed {
    fn layout() -> MoveStructLayout;
}

impl MoveLayout for ID {
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

impl MoveLayout for UID {
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
