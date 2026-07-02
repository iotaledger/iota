// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use iota_sdk_types::{StructTag, TypeTag};
use iota_types::parse_iota_struct_tag;

use crate::error::IotaRpcInputError;

pub fn parse_to_struct_tag(coin_type: &str) -> Result<StructTag, IotaRpcInputError> {
    parse_iota_struct_tag(coin_type)
        .map_err(|e| IotaRpcInputError::CannotParseIotaStructTag(format!("{e}")))
}

pub fn parse_to_type_tag(coin_type: Option<String>) -> Result<TypeTag, IotaRpcInputError> {
    Ok(TypeTag::Struct(Box::new(match coin_type {
        Some(c) => parse_to_struct_tag(&c)?,
        None => StructTag::new_gas(),
    })))
}
