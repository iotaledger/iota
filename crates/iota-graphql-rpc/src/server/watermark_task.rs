// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{mem, sync::Arc, time::Duration};

use async_graphql::ServerError;
use diesel::{ExpressionMethods, OptionalExtension, QueryDsl};
use iota_indexer::schema::checkpoints;
use tokio::sync::{OnceCell, RwLock, watch};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::{
    data::{Db, DbConnection, QueryExecutor},
    error::Error,
    metrics::Metrics,
    types::chain_identifier::ChainIdentifier,
};

/// Watermark task that periodically updates the current checkpoint, checkpoint
/// timestamp, and epoch values.
pub(crate) struct WatermarkTask {
    /// Thread-safe watermark that avoids writer starvation
    watermark: WatermarkLock,
    /// Cached chain identifier.
    chain_identifier: ChainIdentifierCache,
    db: Db,
    metrics: Metrics,
    sleep: Duration,
    cancel: CancellationToken,
    sender: watch::Sender<u64>,
    receiver: watch::Receiver<u64>,
}

pub(crate) type WatermarkLock = Arc<RwLock<Watermark>>;

/// Cache the chain identifier with guaranteed one-time initialization. Once
/// set, typically from database, the value cannot be changed.
#[derive(Clone, Default)]
pub(crate) struct ChainIdentifierCache(pub(crate) Arc<OnceCell<ChainIdentifier>>);

impl ChainIdentifierCache {
    /// Read the stored chain identifier.
    pub(crate) fn read(&self) -> ChainIdentifier {
        self.0.get().copied().unwrap_or_default()
    }
}

/// Watermark used by GraphQL queries to ensure cross-query consistency and flag
/// epoch-boundary changes.
#[derive(Clone, Copy, Default)]
pub(crate) struct Watermark {
    /// The checkpoint upper-bound for the query.
    pub checkpoint: u64,
    /// The checkpoint upper-bound timestamp for the query.
    pub checkpoint_timestamp_ms: u64,
    /// The current epoch.
    pub epoch: u64,
}

/// Starts an infinite loop that periodically updates the `checkpoint_viewed_at`
/// high watermark.
impl WatermarkTask {
    pub(crate) fn new(
        db: Db,
        metrics: Metrics,
        sleep: Duration,
        cancel: CancellationToken,
    ) -> Self {
        let (sender, receiver) = watch::channel(0);

        Self {
            watermark: Default::default(),
            chain_identifier: Default::default(),
            db,
            metrics,
            sleep,
            cancel,
            sender,
            receiver,
        }
    }

    pub(crate) async fn run(&self) {
        // start the process of finding & setting the chain identifier
        // so that it can be used in all requests.
        self.initialize_chain_identifier().await;

        let mut interval = tokio::time::interval(self.sleep);
        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => {
                    info!("shutdown signal received, terminating watermark update task");
                    return;
                },
                _ = interval.tick() => {
                    let Watermark {checkpoint, epoch, checkpoint_timestamp_ms } = match Watermark::query(&self.db).await {
                        Ok(Some(watermark)) => watermark,
                        Ok(None) => continue,
                        Err(e) => {
                            error!("error fetching the watermark: {e}");
                            self.metrics.inc_errors(&[ServerError::new(e.to_string(), None)]);
                            continue;
                        }
                    };

                    // Write the watermark as follows to limit how long we hold the lock
                    let prev_epoch = {
                        let mut w = self.watermark.write().await;
                        w.checkpoint = checkpoint;
                        w.checkpoint_timestamp_ms = checkpoint_timestamp_ms;
                        mem::replace(&mut w.epoch, epoch)
                    };

                    if epoch > prev_epoch {
                        self.sender.send(epoch).unwrap();
                    }
                }
            }
        }
    }

    /// Returns a clone of the watermark lock.
    ///
    /// It clones the underlying `Arc<RwLock<Watermark>>` wrapper, which means
    /// the returned `WatermarkLock` shares the same inner data with the
    /// original.
    pub(crate) fn lock(&self) -> WatermarkLock {
        self.watermark.clone()
    }

    /// Returns a clone of the chain identifier cache.
    ///
    /// It clones the underlying `Arc<OnceCell<ChainIdentifier>>` wrapper, which
    /// means the returned `ChainIdentifierCache` shares the same inner data
    /// with the original.
    pub(crate) fn chain_id_cache(&self) -> ChainIdentifierCache {
        self.chain_identifier.clone()
    }

    /// Receiver for subscribing to epoch changes.
    pub(crate) fn epoch_receiver(&self) -> watch::Receiver<u64> {
        self.receiver.clone()
    }

    /// Initialize the chain identifier if not already initialized.
    ///
    /// This ensures it is initialized only once, regardless of how many times
    /// this method is called concurrently.
    async fn initialize_chain_identifier(&self) {
        let mut interval = tokio::time::interval(self.sleep);
        self.chain_identifier.0.get_or_init(|| async {
            loop {
                tokio::select! {
                    _ = self.cancel.cancelled() => {
                        info!("shutdown signal received, terminating attempt to get chain identifier");
                        // return a default in case of cancellation
                        return ChainIdentifier::default();
                    },
                    _ = interval.tick() => {
                        match ChainIdentifier::query(&self.db).await {
                            Ok(Some(chain)) => return chain.into(),
                            Ok(None) => continue,
                            Err(e) => {
                                error!("failed to fetch chain identifier: {e}");
                                self.metrics.inc_errors(&[ServerError::new(e.to_string(), None)]);
                                continue;
                            }
                        }
                    }
                }
            }
        }).await;
    }
}

impl Watermark {
    pub(crate) async fn new(lock: WatermarkLock) -> Self {
        let w = lock.read().await;
        Self {
            checkpoint: w.checkpoint,
            checkpoint_timestamp_ms: w.checkpoint_timestamp_ms,
            epoch: w.epoch,
        }
    }

    pub(crate) async fn query(db: &Db) -> Result<Option<Watermark>, Error> {
        use checkpoints::dsl;
        let Some((checkpoint, checkpoint_timestamp_ms, epoch)): Option<(i64, i64, i64)> = db
            .execute(move |conn| {
                conn.first(move || {
                    dsl::checkpoints
                        .select((dsl::sequence_number, dsl::timestamp_ms, dsl::epoch))
                        .order_by(dsl::sequence_number.desc())
                })
                .optional()
            })
            .await
            .map_err(|e| Error::Internal(format!("Failed to fetch checkpoint: {e}")))?
        else {
            return Ok(None);
        };
        Ok(Some(Watermark {
            checkpoint: checkpoint as u64,
            checkpoint_timestamp_ms: checkpoint_timestamp_ms as u64,
            epoch: epoch as u64,
        }))
    }
}
