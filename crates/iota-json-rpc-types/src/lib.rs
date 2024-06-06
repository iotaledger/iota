// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub use balance_changes::*;
pub use iota_checkpoint::*;
pub use iota_coin::*;
pub use iota_event::*;
pub use iota_extended::*;
pub use iota_governance::*;
pub use iota_move::*;
pub use iota_object::*;
pub use iota_protocol::*;
pub use iota_transaction::*;
use iota_types::{base_types::ObjectID, dynamic_field::DynamicFieldInfo};
pub use object_changes::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(test)]
#[path = "unit_tests/rpc_types_tests.rs"]
mod rpc_types_tests;

mod balance_changes;
mod displays;
mod iota_checkpoint;
mod iota_coin;
mod iota_event;
mod iota_extended;
mod iota_governance;
mod iota_move;
mod iota_object;
mod iota_protocol;
mod iota_transaction;
mod object_changes;

pub type DynamicFieldPage = Page<DynamicFieldInfo, ObjectID>;
/// `next_cursor` points to the last item in the page;
/// Reading with `next_cursor` will start from the next item after `next_cursor`
/// if `next_cursor` is `Some`, otherwise it will start from the first item.
#[derive(Clone, Debug, JsonSchema, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Page<T, C> {
    pub data: Vec<T>,
    pub next_cursor: Option<C>,
    pub has_next_page: bool,
}

impl<T, C> Page<T, C> {
    pub fn empty() -> Self {
        Self {
            data: vec![],
            next_cursor: None,
            has_next_page: false,
        }
    }
}
