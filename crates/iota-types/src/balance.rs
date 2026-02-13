// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::{Identifier, StructTag, TypeTag};
use move_core_types::{
    annotated_value::{MoveFieldLayout, MoveStructLayout, MoveTypeLayout},
    ident_str,
};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::{
    error::{ExecutionError, ExecutionErrorKind},
    iota_sdk_types_conversions::{struct_tag_core_to_sdk, struct_tag_sdk_to_core},
    iota_serde::{BigInt, Readable},
};
pub const BALANCE_CREATE_REWARDS_FUNCTION_NAME: Identifier =
    Identifier::from_static("create_staking_rewards");
pub const BALANCE_DESTROY_REBATES_FUNCTION_NAME: Identifier =
    Identifier::from_static("destroy_storage_rebates");

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct Supply {
    #[serde_as(as = "Readable<BigInt<u64>, _>")]
    pub value: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct Balance {
    value: u64,
}

impl Balance {
    pub fn new(value: u64) -> Self {
        Self { value }
    }

    pub fn withdraw(&mut self, amount: u64) -> Result<(), ExecutionError> {
        fp_ensure!(
            self.value >= amount,
            ExecutionError::new_with_source(
                ExecutionErrorKind::InsufficientCoinBalance,
                format!("balance: {} required: {}", self.value, amount)
            )
        );
        self.value -= amount;
        Ok(())
    }

    pub fn deposit_for_safe_mode(&mut self, amount: u64) {
        self.value += amount;
    }

    pub fn value(&self) -> u64 {
        self.value
    }

    pub fn to_bcs_bytes(&self) -> Vec<u8> {
        bcs::to_bytes(&self).unwrap()
    }

    pub fn layout(type_param: TypeTag) -> MoveStructLayout {
        MoveStructLayout {
            type_: struct_tag_sdk_to_core(&StructTag::new_balance(type_param)),
            fields: vec![MoveFieldLayout::new(
                ident_str!("value").to_owned(),
                MoveTypeLayout::U64,
            )],
        }
    }

    /// Check if a struct layout represents a `Balance<T>` type with the
    /// expected field structure.
    pub fn is_balance_layout(struct_layout: &MoveStructLayout) -> bool {
        let ty = struct_tag_core_to_sdk(&struct_layout.type_);

        if !ty.is_balance() {
            return false;
        }

        if ty.type_params().len() != 1 {
            return false;
        }

        if struct_layout.fields.len() != 1 {
            return false;
        }

        let Some(field) = struct_layout.fields.first() else {
            return false;
        };

        if field.name.as_str() != "value" {
            return false;
        }

        if !matches!(field.layout, MoveTypeLayout::U64) {
            return false;
        }

        true
    }
}
