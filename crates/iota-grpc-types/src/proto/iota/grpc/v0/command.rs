// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.command.rs");
include!("../../../generated/iota.grpc.v0.command.field_info.rs");
include!("../../../generated/iota.grpc.v0.command.accessors.rs");

use iota_types::{TypeTag, transaction::Argument as IotaArgument};

use super::types::{TypeTagVector, type_tag};

impl From<&iota_types::TypeTag> for super::types::TypeTag {
    fn from(ty: &iota_types::TypeTag) -> Self {
        let type_tag = match ty {
            TypeTag::Bool => type_tag::TypeTag::BoolTag(true),
            TypeTag::U8 => type_tag::TypeTag::U8Tag(true),
            TypeTag::U16 => type_tag::TypeTag::U16Tag(true),
            TypeTag::U32 => type_tag::TypeTag::U32Tag(true),
            TypeTag::U64 => type_tag::TypeTag::U64Tag(true),
            TypeTag::U128 => type_tag::TypeTag::U128Tag(true),
            TypeTag::U256 => type_tag::TypeTag::U256Tag(true),
            TypeTag::Address => type_tag::TypeTag::AddressTag(true),
            TypeTag::Signer => type_tag::TypeTag::SignerTag(true),
            TypeTag::Vector(inner) => type_tag::TypeTag::VectorTag(Box::new(TypeTagVector {
                inner_type: Some(Box::new(inner.as_ref().into())),
            })),
            TypeTag::Struct(struct_tag) => {
                type_tag::TypeTag::StructTag(super::types::TypeTagStruct {
                    struct_tag: struct_tag.to_canonical_string(true),
                })
            }
        };

        Self {
            type_tag: Some(type_tag),
        }
    }
}

impl From<iota_types::transaction::Argument> for Argument {
    fn from(arg: iota_types::transaction::Argument) -> Self {
        let kind = match arg {
            IotaArgument::GasCoin => argument::Kind::GasCoin(argument::GasCoin {}),
            IotaArgument::Input(idx) => argument::Kind::Input(argument::Input {
                index: Some(idx as u32),
            }),
            IotaArgument::Result(idx) => argument::Kind::Result(argument::Result {
                index: Some(idx as u32),
                nested_result_index: None,
            }),
            IotaArgument::NestedResult(idx, nested_idx) => {
                argument::Kind::Result(argument::Result {
                    index: Some(idx as u32),
                    nested_result_index: Some(nested_idx as u32),
                })
            }
        };

        Self { kind: Some(kind) }
    }
}
