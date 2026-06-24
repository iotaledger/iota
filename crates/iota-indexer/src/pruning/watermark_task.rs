// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Background task that periodically fetches pruning watermarks from the
//! database and maintains them in memory for RPC reads to check data
//! availability.

use std::{collections::HashMap, sync::Arc, time::Duration};

use arc_swap::ArcSwap;
use iota_metrics::spawn_monitored_task;
use tokio::time::interval;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::{
    errors::IndexerError,
    ingestion::common::persist::CommitterTables,
    models::watermarks::StoredWatermark,
    store::{IndexerStore, PgIndexerStore},
};

/// How often to refresh watermarks from the database
const WATERMARK_UPDATE_INTERVAL: Duration = Duration::from_secs(5);

/// In-memory cache of pruning watermarks
///
/// Provides fast access to watermark data without querying the database on
/// every read. A background task periodically refreshes the cache from the
/// database.
///
/// Uses [`ArcSwap`] for lock-free reads to avoid write starvation under high
/// read load. Cloned instances share the same underlying data and will reflect
/// updates from the background task.
#[derive(Clone)]
pub struct WatermarkCache {
    inner: Arc<ArcSwap<WatermarkCacheInner>>,
}

/// Map from entity name to its watermark data
type WatermarkCacheInner = HashMap<String, StoredWatermark>;

impl Default for WatermarkCache {
    fn default() -> Self {
        Self {
            inner: Arc::new(ArcSwap::from_pointee(HashMap::new())),
        }
    }
}

impl WatermarkCache {
    /// Creates a new empty watermark cache
    pub fn new() -> Self {
        Self::default()
    }

    /// Gets the watermark for a specific entity
    /// Returns `None` if the entity has no watermark
    pub fn get(&self, entity: CommitterTables) -> Option<StoredWatermark> {
        let cache = self.inner.load();
        cache.get(entity.as_ref()).cloned()
    }

    /// Gets the lowest checkpoint that is available for all specified tables.
    /// Returns `None` if no watermarks are available for any of the tables.
    pub fn get_lowest_available_cp_for_tables(&self, tables: &[CommitterTables]) -> Option<i64> {
        let cache = self.inner.load();
        tables
            .iter()
            .filter_map(|table| cache.get(table.as_ref()).map(|wm| wm.min_available_cp))
            .max()
    }

    /// Gets the lowest transaction that is available for all specified tables.
    /// Returns `None` if no watermarks are available for any of the tables.
    pub fn get_lowest_available_tx_for_tables(&self, tables: &[CommitterTables]) -> Option<i64> {
        let cache = self.inner.load();
        tables
            .iter()
            .filter_map(|table| cache.get(table.as_ref()).map(|wm| wm.min_available_tx))
            .max()
    }

    /// Updates the cache with fresh watermarks from the database
    ///
    /// Uses [`ArcSwap`] for lock-free reads - writes create new Arc instead of
    /// blocking readers
    fn update(&self, watermarks: Vec<StoredWatermark>) {
        let mut new_watermarks = HashMap::new();
        for watermark in watermarks {
            new_watermarks.insert(watermark.entity.clone(), watermark);
        }

        self.inner.store(Arc::new(new_watermarks));
    }
}

/// Background task that periodically updates the watermark cache
pub struct WatermarkTask {
    store: PgIndexerStore,
    cache: WatermarkCache,
    update_interval: Duration,
}

impl WatermarkTask {
    /// Creates a new watermark task with the given cache
    pub fn new(store: PgIndexerStore, cache: WatermarkCache) -> Self {
        Self {
            store,
            cache,
            update_interval: WATERMARK_UPDATE_INTERVAL,
        }
    }

    /// Starts the background task that updates watermarks
    pub fn start(self, cancel: CancellationToken) {
        spawn_monitored_task!(async move {
            self.run(cancel).await;
        });
    }

    /// Runs the watermark update loop
    async fn run(self, cancel: CancellationToken) {
        info!("Starting watermark update task");
        let mut interval = interval(self.update_interval);
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    info!("Watermark update task cancelled");
                    break;
                }
                _ = interval.tick() => {
                    if let Err(e) = self.update_watermarks().await {
                        error!("Failed to update watermarks: {e}");
                    }
                }
            }
        }
    }

    /// Fetches watermarks from the database and updates the cache
    async fn update_watermarks(&self) -> Result<(), IndexerError> {
        let (watermarks, _timestamp_ms) = self.store.get_watermarks().await?;

        if !watermarks.is_empty() {
            info!("Updated {} watermarks from database", watermarks.len());
        }

        self.cache.update(watermarks);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_watermark_cache() {
        let cache = WatermarkCache::new();

        // Initially empty
        assert!(cache.get(CommitterTables::Transactions).is_none());

        let watermark = StoredWatermark {
            entity: CommitterTables::Transactions.as_ref().to_string(),
            current_epoch: 100,
            max_committed_cp: 1000,
            max_committed_tx: 10000,
            min_available_epoch: 50,
            min_bounds_updated_at_timestamp_ms: 123456789,
            lowest_unpruned_key: 400,
            min_available_tx: 5000,
            min_available_cp: 500,
        };

        cache.update(vec![watermark]);

        let retrieved = cache.get(CommitterTables::Transactions).unwrap();
        assert_eq!(retrieved.min_available_cp, 500);
        assert_eq!(retrieved.min_available_tx, 5000);
        assert_eq!(retrieved.min_available_epoch, 50);
    }
}
