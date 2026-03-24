// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use move_binary_format::errors::{PartialVMError, PartialVMResult};
use move_core_types::{
    gas_algebra::AbstractMemorySize,
    runtime_value::{MoveStructLayout, MoveTypeLayout},
    vm_status::StatusCode,
};
use move_vm_types::values::{GlobalValue, Value};
use serde::{Serialize, de::DeserializeOwned};

/// Serializes a Rust value into a Move global value(Struct field) according to
/// the provided MoveTypeLayout.
/// Returns the GlobalValue and its size in bytes.
pub fn to_global_value<T: ?Sized + Serialize>(
    field: &T,
    field_move_layout: MoveTypeLayout,
) -> PartialVMResult<(GlobalValue, AbstractMemorySize)> {
    let move_layout = struct_layout_with_field(field_move_layout);

    let move_value = to_value(field, &move_layout)?;
    let move_value_size = move_value.legacy_size();

    Ok((
        GlobalValue::cached(move_value).expect("Failed to cache global value"),
        move_value_size,
    ))
}

/// Serializes a Rust value into a Move value according to the provided
/// MoveTypeLayout.
pub fn to_value<T: ?Sized + Serialize>(
    input: &T,
    input_move_layout: &MoveTypeLayout,
) -> PartialVMResult<Value> {
    let bytes = bcs::to_bytes(input).map_err(|err| {
        PartialVMError::new(StatusCode::VALUE_SERIALIZATION_ERROR)
            .with_message(format!("Failed to serialize an input: {err}"))
    })?;
    Value::simple_deserialize(&bytes, input_move_layout).ok_or_else(|| {
        PartialVMError::new(StatusCode::UNEXPECTED_DESERIALIZATION_ERROR)
            .with_message("Failed to deserialize an input to a Move value".to_string())
    })
}

/// Deserializes a Move value into a Rust value according to the provided
/// MoveTypeLayout.
pub fn from_value<T: DeserializeOwned>(
    value: Value,
    value_move_layout: &MoveTypeLayout,
) -> PartialVMResult<T> {
    let bytes = value.simple_serialize(value_move_layout).ok_or_else(|| {
        PartialVMError::new(StatusCode::VALUE_SERIALIZATION_ERROR)
            .with_message("Failed to serialize a value".to_string())
    })?;
    bcs::from_bytes::<T>(&bytes).map_err(|err| {
        PartialVMError::new(StatusCode::UNEXPECTED_DESERIALIZATION_ERROR)
            .with_message(format!("Failed to deserialize a value: {err}"))
    })
}

/// Helper function to create a MoveTypeLayout for a struct with a single field
/// of the given layout.
fn struct_layout_with_field(field: MoveTypeLayout) -> MoveTypeLayout {
    MoveTypeLayout::Struct(Box::new(MoveStructLayout(Box::new(vec![field]))))
}
