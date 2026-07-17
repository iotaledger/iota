// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use serde::Deserialize;

use crate::{
    collection_types::VecMap,
    id::{ID, UID},
};

// TODO: add tests to keep in sync
/// Rust version of the Move iota::display::Display type
#[derive(Debug, Deserialize, Clone, Eq, PartialEq)]
pub struct DisplayObject {
    pub id: UID,
    pub fields: VecMap<String, String>,
    pub version: u16,
}

#[derive(Deserialize, Debug)]
/// The event that is emitted when a `Display` version is "released".
/// Serves for Display versioning.
pub struct DisplayVersionUpdatedEvent {
    pub id: ID,
    pub version: u16,
    pub fields: VecMap<String, String>,
}
