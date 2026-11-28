// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_move_types::iota_framework::dynamic_field::Field;
use iota_sdk_types::MoveStruct;
pub use iota_sdk_types::iota_names::registry::*;

use crate::{error::IotaNamesError, name::Name};

pub fn name_record_from_move_struct(
    move_struct: &MoveStruct,
) -> Result<NameRecord, IotaNamesError> {
    move_struct
        .to_rust::<Field<Name, NameRecord>>()
        .map(|record| record.value)
        .map_err(|_| IotaNamesError::MalformedObject(move_struct.id()))
}
