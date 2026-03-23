// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::{
    base_types::{Identifier, TypeTag},
    error::{ExecutionError, ExecutionErrorKind},
};

#[macro_use]
extern crate iota_types;

pub mod adapter;
pub mod error;
pub mod execution_engine;
pub mod execution_mode;
pub mod execution_value;
pub mod gas_charger;
pub mod gas_meter;
pub mod programmable_transactions;
pub mod temporary_store;
pub mod type_layout_resolver;
pub mod type_resolver;

pub(crate) fn validate_type_tag(tag: &TypeTag) -> Result<(), ExecutionError> {
    match tag {
        TypeTag::Bool
        | TypeTag::U8
        | TypeTag::U16
        | TypeTag::U32
        | TypeTag::U64
        | TypeTag::U128
        | TypeTag::U256
        | TypeTag::Address
        | TypeTag::Signer => Ok(()),
        TypeTag::Vector(inner) => validate_type_tag(inner),
        TypeTag::Struct(struct_tag) => {
            Identifier::new(struct_tag.module().as_str())
                .and(Identifier::new(struct_tag.name().as_str()))
                .map_err(|e| {
                    ExecutionError::new_with_source(
                        ExecutionErrorKind::VmInvariantViolation,
                        e.to_string(),
                    )
                })?;
            for tag in struct_tag.type_params() {
                validate_type_tag(tag)?;
            }
            Ok(())
        }
    }
}
