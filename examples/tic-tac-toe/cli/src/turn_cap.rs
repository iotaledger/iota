// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::base_types::ObjectID;
use serde::Deserialize;

/// Rust representation of a Move `owned::TurnCap`, suitable for deserializing
/// from their BCS representation.
#[derive(Deserialize)]
pub(crate) struct TurnCap {
    pub _id: ObjectID,
    pub game: ObjectID,
}
