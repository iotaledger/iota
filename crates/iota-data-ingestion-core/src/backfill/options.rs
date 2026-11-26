// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{num::NonZeroUsize, time::Duration};

use backoff::ExponentialBackoff;

use crate::backfill::order::IngestionOrder;

/// Options for configuring a [`BackfillExecutor`](crate::BackfillExecutor).
#[derive(Clone, Debug)]
pub struct BackfillOptions {
    /// The order in which the range is ingested.
    pub(crate) order: IngestionOrder,
    /// Number of workers that process checkpoints in parallel.
    pub(crate) worker_concurrency: usize,
    /// Retry policy for processing a checkpoint and for committing a batch.
    pub(crate) backoff: ExponentialBackoff,
    /// Time the store may take to return the next checkpoint.
    pub(crate) timeout: Duration,
    /// Number of downloads the store may run at once.
    pub(crate) download_concurrency: usize,
}

impl BackfillOptions {
    /// Sets the order in which the range is ingested.
    ///
    /// Default: [`IngestionOrder::Ascending`].
    pub fn with_order(mut self, order: IngestionOrder) -> Self {
        self.order = order;
        self
    }

    /// Sets the number of workers that process checkpoints in parallel.
    ///
    /// Default: 1.
    pub fn with_worker_concurrency(mut self, worker_concurrency: NonZeroUsize) -> Self {
        self.worker_concurrency = worker_concurrency.get();
        self
    }

    /// Sets the retry policy for processing a checkpoint and for committing a batch.
    ///
    ///  Default: [`ExponentialBackoff::default`].
    pub fn with_backoff(mut self, backoff: ExponentialBackoff) -> Self {
        self.backoff = backoff;
        self
    }

    /// Sets the time the store may take to return the next checkpoint.
    ///
    /// Default: 30 seconds.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sets the number of downloads the store may run at once.
    ///
    /// Default: 10.
    /// # Note
    /// Only the historical store takes this value for now.
    pub fn with_download_concurrency(mut self, download_concurrency: NonZeroUsize) -> Self {
        self.download_concurrency = download_concurrency.get();
        self
    }
}

impl Default for BackfillOptions {
    fn default() -> Self {
        Self {
            order: IngestionOrder::default(),
            worker_concurrency: 1,
            backoff: ExponentialBackoff::default(),
            timeout: Duration::from_secs(30),
            download_concurrency: 10,
        }
    }
}
