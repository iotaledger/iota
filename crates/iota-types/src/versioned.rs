// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

use crate::id::UID;

/// Rust version of the Move iota::versioned::Versioned type.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct Versioned {
    pub id: UID,
    pub version: u64,
}
