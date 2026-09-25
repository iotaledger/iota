// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::ObjectId;
use serde::{Deserialize, Serialize};

use crate::id::UID;

/// Rust version of the Move iota::table::Table type.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct TableVec {
    pub contents: Table,
}

impl Default for TableVec {
    fn default() -> Self {
        TableVec {
            contents: Table {
                id: ObjectId::ZERO,
                size: 0,
            },
        }
    }
}

/// Rust version of the Move iota::table::Table type.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct Table {
    pub id: ObjectId,
    pub size: u64,
}

impl Default for Table {
    fn default() -> Self {
        Table {
            id: ObjectId::ZERO,
            size: 0,
        }
    }
}

/// Rust version of the Move iota::bag::Bag type.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct Bag {
    pub id: UID,
    pub size: u64,
}

impl Default for Bag {
    fn default() -> Self {
        Self {
            id: UID::new(ObjectId::ZERO),
            size: 0,
        }
    }
}
