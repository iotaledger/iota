// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::TypeTag;
use iota_types::error::ExecutionError;
use move_vm_types::loaded_data::runtime_types::Type;

pub trait TypeTagResolver {
    fn get_type_tag(&self, type_: &Type) -> Result<TypeTag, ExecutionError>;
}
