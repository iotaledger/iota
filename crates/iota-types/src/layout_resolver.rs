// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::{StructTag, TypeTag};
use move_bytecode_utils::{layout::TypeLayoutBuilder, module_cache::GetModule};
use move_core_types::annotated_value as A;

use crate::{error::IotaError, iota_sdk_types_conversions::type_tag_sdk_to_core};

pub trait LayoutResolver {
    fn get_annotated_layout(
        &mut self,
        struct_tag: &StructTag,
    ) -> Result<A::MoveDatatypeLayout, IotaError>;
}

pub fn get_layout_from_struct_tag(
    struct_tag: StructTag,
    resolver: &impl GetModule,
) -> Result<A::MoveDatatypeLayout, IotaError> {
    let type_ = TypeTag::Struct(Box::new(struct_tag));
    let layout = TypeLayoutBuilder::build_with_types(&type_tag_sdk_to_core(&type_), resolver)
        .map_err(|e| IotaError::ObjectSerialization {
            error: e.to_string(),
        })?;
    match layout {
        A::MoveTypeLayout::Struct(l) => Ok(A::MoveDatatypeLayout::Struct(l)),
        A::MoveTypeLayout::Enum(e) => Ok(A::MoveDatatypeLayout::Enum(e)),
        _ => {
            unreachable!(
                "We called get_layout_from_struct_tag on a datatype, should get a datatype layout"
            )
        }
    }
}

pub fn into_struct_layout(layout: A::MoveDatatypeLayout) -> Result<A::MoveStructLayout, IotaError> {
    match layout {
        A::MoveDatatypeLayout::Struct(s) => Ok(*s),
        A::MoveDatatypeLayout::Enum(e) => Err(IotaError::ObjectSerialization {
            error: format!("Expected struct layout but got an enum {e:?}"),
        }),
    }
}
