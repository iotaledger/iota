// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::ops::RangeInclusive;

use async_trait::async_trait;

use crate::{db::ConnectionPool, errors::IndexerError};

#[async_trait]
pub(crate) trait BackfillTask: Send + Sync {
    /// Backfill the database for a specific range.
    async fn backfill_range(
        &self,
        pool: ConnectionPool,
        range: &RangeInclusive<usize>,
    ) -> Result<(), IndexerError>;
}
