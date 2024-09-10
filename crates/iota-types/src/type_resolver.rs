// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use move_core_types::{
    annotated_value as A,
    language_storage::{StructTag, TypeTag},
};
use move_vm_types::loaded_data::runtime_types::Type;

use crate::error::{ExecutionError, IotaError};

pub trait LayoutResolver {
    fn get_annotated_layout(
        &mut self,
        struct_tag: &StructTag,
    ) -> Result<A::MoveStructLayout, IotaError>;
}

pub trait TypeTagResolver {
    fn get_type_tag(&self, type_: &Type) -> Result<TypeTag, ExecutionError>;
}
