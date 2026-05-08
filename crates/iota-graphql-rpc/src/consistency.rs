// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_graphql::connection::CursorType;
use serde::{Deserialize, Serialize};

use crate::types::cursor::{JsonCursor, ScanLimited};

/// The checkpoint sequence number for entities not available for view.
pub(crate) const UNAVAILABLE_CHECKPOINT_SEQUENCE_NUMBER: u64 = u64::MAX;

/// The consistent cursor for an index into a `Vec` field is constructed from
/// the index of the element and the checkpoint the cursor was constructed at.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub(crate) struct ConsistentIndexCursor {
    #[serde(rename = "i")]
    pub ix: usize,
    /// The checkpoint sequence number at which the entity corresponding to this
    /// cursor was viewed at.
    pub c: u64,
}

/// The consistent cursor for an index into a `Map` field is constructed from
/// the name or key of the element and the checkpoint the cursor was constructed
/// at.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub(crate) struct ConsistentNamedCursor {
    #[serde(rename = "n")]
    pub name: String,
    /// The checkpoint sequence number at which the entity corresponding to this
    /// cursor was viewed at.
    pub c: u64,
}

/// Trait for cursors that have a checkpoint sequence number associated with
/// them.
pub(crate) trait Checkpointed: CursorType {
    fn checkpoint_viewed_at(&self) -> u64;
}

impl Checkpointed for JsonCursor<ConsistentIndexCursor> {
    fn checkpoint_viewed_at(&self) -> u64 {
        self.c
    }
}

impl Checkpointed for JsonCursor<ConsistentNamedCursor> {
    fn checkpoint_viewed_at(&self) -> u64 {
        self.c
    }
}

impl ScanLimited for JsonCursor<ConsistentIndexCursor> {}

impl ScanLimited for JsonCursor<ConsistentNamedCursor> {}
