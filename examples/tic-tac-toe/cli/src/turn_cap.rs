// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::ObjectId;
use serde::Deserialize;

/// Rust representation of a Move `owned::TurnCap`, suitable for deserializing
/// from their BCS representation.
#[derive(Deserialize)]
pub(crate) struct TurnCap {
    pub _id: ObjectId,
    pub game: ObjectId,
}
