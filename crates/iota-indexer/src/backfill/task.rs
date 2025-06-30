// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::ops::RangeInclusive;

use async_trait::async_trait;

use crate::{db::ConnectionPool, errors::IndexerError};

/// Encapsulates the logic to process and persist data for a chunk of
/// checkpoints.
#[async_trait]
pub(crate) trait BackfillTask: Send + Sync {
    async fn backfill_range(
        &self,
        pool: ConnectionPool,
        range: &RangeInclusive<usize>,
    ) -> Result<(), IndexerError>;
}
