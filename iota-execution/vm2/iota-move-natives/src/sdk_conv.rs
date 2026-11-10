// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Conversions between Move core types and the IOTA SDK types used by the
//! execution layer. The upstream adapter relies on `From` impls between these
//! types; IOTA keeps the SDK types independent of the Move crates, so the
//! conversions are explicit here.

use iota_sdk_types::{Address, MoveObjectType, ObjectData, ObjectId};
use iota_types::iota_sdk_types_conversions::{struct_tag_core_to_sdk, struct_tag_sdk_to_core};
use move_core_types::{
    account_address::AccountAddress, language_storage::StructTag as CoreStructTag,
};

pub(crate) trait AccountAddressExt {
    fn to_object_id(self) -> ObjectId;
    fn to_sdk_address(self) -> Address;
}

impl AccountAddressExt for AccountAddress {
    fn to_object_id(self) -> ObjectId {
        ObjectId::new(self.into_bytes())
    }
    fn to_sdk_address(self) -> Address {
        Address::new(self.into_bytes())
    }
}

pub(crate) trait ToAccountAddress {
    fn to_account_address(self) -> AccountAddress;
}

impl ToAccountAddress for AccountAddress {
    fn to_account_address(self) -> AccountAddress {
        self
    }
}

impl ToAccountAddress for ObjectId {
    fn to_account_address(self) -> AccountAddress {
        AccountAddress::new(self.into_bytes())
    }
}

impl ToAccountAddress for &ObjectId {
    fn to_account_address(self) -> AccountAddress {
        AccountAddress::new(self.into_bytes())
    }
}

impl ToAccountAddress for Address {
    fn to_account_address(self) -> AccountAddress {
        AccountAddress::new(self.into_bytes())
    }
}

impl ToAccountAddress for &Address {
    fn to_account_address(self) -> AccountAddress {
        AccountAddress::new(self.into_bytes())
    }
}

pub(crate) fn move_object_type(tag: CoreStructTag) -> MoveObjectType {
    MoveObjectType::new(struct_tag_core_to_sdk(&tag))
}

pub(crate) fn core_struct_tag(ty: &MoveObjectType) -> CoreStructTag {
    struct_tag_sdk_to_core(ty.struct_tag())
}

pub(crate) fn try_as_move(data: &ObjectData) -> Option<&iota_sdk_types::MoveStruct> {
    match data {
        ObjectData::Struct(s) => Some(s),
        _ => None,
    }
}
