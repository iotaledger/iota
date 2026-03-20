// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! RocksDB per-epoch persistence using typed-store.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use typed_store::{
    Map,
    rocks::{DBMap, MetricConf, ReadWriteOptions, open_cf},
};

/// Compact block for storage — no string fields, BCS-serialized.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredBlock {
    pub round: u32,
    pub author: u8,
    pub digest: [u8; 32],
    pub timestamp_ms: u64,
    /// Ancestors as (round, author) — digest omitted since there's
    /// one block per (round, author) slot.
    pub ancestors: Vec<(u32, u8)>,
    /// Acknowledgments: block refs acknowledged by this block at accept time.
    pub acknowledgments: Vec<(u32, u8, [u8; 32])>,
}

/// Compact leader for storage.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredLeader {
    pub wave: u32,
    pub leader_round: u32,
    pub leader_authority: u8,
    pub status: u8,
    pub block_digest: Option<[u8; 32]>,
}

/// Stored committee info.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredCommittee {
    pub epoch: u64,
    pub total_stake: u64,
    pub quorum_threshold: u64,
    pub validators: Vec<StoredValidator>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredValidator {
    pub index: u8,
    pub hostname: String,
    pub stake: u64,
}

/// Metadata tags for the metadata column family.
const META_COMMITTEE: u8 = 0;
const META_LAST_ROUND: u8 = 1;
const META_STATUS: u8 = 2;
const META_FIRST_ROUND: u8 = 3;

/// Stored status (latest known).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredStatus {
    pub highest_accepted_round: u32,
    pub last_commit_index: u32,
    pub last_commit_round: u32,
    pub num_authorities: u32,
}

/// A single epoch's RocksDB database.
pub struct EpochStore {
    /// Key: (round, author), Value: BCS-encoded StoredBlock
    blocks: DBMap<(u32, u8), StoredBlock>,
    /// Key: leader_round, Value: BCS-encoded StoredLeader.
    /// Each round has at most one leader slot in the leader schedule, so
    /// round is a unique key for leader decisions.
    leaders: DBMap<u32, StoredLeader>,
    /// Key: metadata tag, Value: BCS-encoded metadata
    metadata: DBMap<u8, Vec<u8>>,
}

impl EpochStore {
    pub fn open(data_dir: &Path, epoch: u64) -> Self {
        let path = data_dir.join(format!("epoch-{epoch}"));
        std::fs::create_dir_all(&path).expect("Failed to create epoch directory");

        let cfs = ["blocks", "leaders", "metadata"];
        let db = open_cf(&path, None, MetricConf::new("dag_viz"), &cfs)
            .expect("Failed to open epoch RocksDB");

        let blocks = DBMap::reopen(&db, Some("blocks"), &ReadWriteOptions::default(), false)
            .expect("Failed to open blocks CF");
        let leaders = DBMap::reopen(&db, Some("leaders"), &ReadWriteOptions::default(), false)
            .expect("Failed to open leaders CF");
        let metadata = DBMap::reopen(&db, Some("metadata"), &ReadWriteOptions::default(), false)
            .expect("Failed to open metadata CF");

        Self {
            blocks,
            leaders,
            metadata,
        }
    }

    /// Open for tests with serialized metrics initialization to avoid
    /// Prometheus registration race conditions when tests run in parallel.
    #[cfg(test)]
    pub fn open_for_test(data_dir: &Path) -> Self {
        // Force-initialize the global DBMetrics singleton before any parallel
        // RocksDB access, avoiding a race in typed_store::metrics::DBMetrics::get().
        use std::sync::Once;
        static INIT_METRICS: Once = Once::new();
        INIT_METRICS.call_once(|| {
            let _ = typed_store::metrics::DBMetrics::get();
        });

        std::fs::create_dir_all(data_dir).expect("Failed to create test directory");
        let cfs = ["blocks", "leaders", "metadata"];
        let db = open_cf(data_dir, None, MetricConf::default(), &cfs)
            .expect("Failed to open test RocksDB");

        let blocks = DBMap::reopen(&db, Some("blocks"), &ReadWriteOptions::default(), false)
            .expect("Failed to open blocks CF");
        let leaders = DBMap::reopen(&db, Some("leaders"), &ReadWriteOptions::default(), false)
            .expect("Failed to open leaders CF");
        let metadata = DBMap::reopen(&db, Some("metadata"), &ReadWriteOptions::default(), false)
            .expect("Failed to open metadata CF");

        Self {
            blocks,
            leaders,
            metadata,
        }
    }

    pub fn insert_block(&self, block: &StoredBlock) {
        self.blocks
            .insert(&(block.round, block.author), block)
            .expect("Failed to insert block");
    }

    pub fn insert_leader(&self, leader: &StoredLeader) {
        self.leaders
            .insert(&leader.leader_round, leader)
            .expect("Failed to insert leader");
    }

    pub fn get_blocks_in_range(&self, from: u32, to: u32) -> Vec<StoredBlock> {
        self.blocks
            .safe_range_iter((from, 0)..=(to, u8::MAX))
            .filter_map(|r| r.ok())
            .map(|(_, v)| v)
            .collect()
    }

    pub fn get_leaders_in_range(&self, from: u32, to: u32) -> Vec<StoredLeader> {
        self.leaders
            .safe_range_iter(from..=to)
            .filter_map(|r| r.ok())
            .map(|(_, v)| v)
            .collect()
    }

    pub fn set_committee(&self, committee: &StoredCommittee) {
        let data = bcs::to_bytes(committee).expect("Failed to serialize committee");
        self.metadata
            .insert(&META_COMMITTEE, &data)
            .expect("Failed to store committee");
    }

    pub fn get_committee(&self) -> Option<StoredCommittee> {
        self.metadata
            .get(&META_COMMITTEE)
            .ok()
            .flatten()
            .and_then(|data| bcs::from_bytes(&data).ok())
    }

    pub fn set_status(&self, status: &StoredStatus) {
        let data = bcs::to_bytes(status).expect("Failed to serialize status");
        self.metadata
            .insert(&META_STATUS, &data)
            .expect("Failed to store status");
    }

    pub fn get_status(&self) -> Option<StoredStatus> {
        self.metadata
            .get(&META_STATUS)
            .ok()
            .flatten()
            .and_then(|data| bcs::from_bytes(&data).ok())
    }

    pub fn set_last_round(&self, round: u32) {
        let data = bcs::to_bytes(&round).expect("Failed to serialize round");
        self.metadata
            .insert(&META_LAST_ROUND, &data)
            .expect("Failed to store last round");
    }

    pub fn get_last_round(&self) -> u32 {
        self.metadata
            .get(&META_LAST_ROUND)
            .ok()
            .flatten()
            .and_then(|data| bcs::from_bytes(&data).ok())
            .unwrap_or(0)
    }

    pub fn set_first_round(&self, round: u32) {
        let data = bcs::to_bytes(&round).expect("Failed to serialize round");
        self.metadata
            .insert(&META_FIRST_ROUND, &data)
            .expect("Failed to store first round");
    }

    pub fn get_first_round(&self) -> u32 {
        self.metadata
            .get(&META_FIRST_ROUND)
            .ok()
            .flatten()
            .and_then(|data| bcs::from_bytes(&data).ok())
            .unwrap_or(0)
    }
}

/// Manages per-epoch stores and handles epoch transitions.
pub struct StorageManager {
    data_dir: PathBuf,
    max_epochs: usize,
    /// Currently open epoch stores, keyed by epoch number.
    stores: RwLock<BTreeMap<u64, Arc<EpochStore>>>,
}

impl StorageManager {
    pub fn new(data_dir: PathBuf, max_epochs: usize) -> Self {
        std::fs::create_dir_all(&data_dir).expect("Failed to create data directory");
        Self {
            data_dir,
            max_epochs,
            stores: RwLock::new(BTreeMap::new()),
        }
    }

    /// Get or create the store for a given epoch.
    pub fn get_or_create_epoch(&self, epoch: u64) -> Arc<EpochStore> {
        {
            let stores = self.stores.read();
            if let Some(store) = stores.get(&epoch) {
                return store.clone();
            }
        }
        let mut stores = self.stores.write();
        // Re-check after acquiring write lock (another thread may have created it)
        if let Some(store) = stores.get(&epoch) {
            return store.clone();
        }
        let store = Arc::new(EpochStore::open(&self.data_dir, epoch));
        stores.insert(epoch, store.clone());

        // Prune old epochs (always keep at least the current one)
        while stores.len() > self.max_epochs.max(1) {
            if let Some((&oldest_epoch, _)) = stores.first_key_value() {
                stores.remove(&oldest_epoch);
                let path = self.data_dir.join(format!("epoch-{oldest_epoch}"));
                if path.exists() {
                    let _ = std::fs::remove_dir_all(&path);
                }
            }
        }

        store
    }

    /// Test-only variant that skips Prometheus metric registration.
    #[cfg(test)]
    pub fn get_or_create_epoch_for_test(&self, epoch: u64) -> Arc<EpochStore> {
        {
            let stores = self.stores.read();
            if let Some(store) = stores.get(&epoch) {
                return store.clone();
            }
        }
        let mut stores = self.stores.write();
        // Re-check after acquiring write lock (another thread may have created it)
        if let Some(store) = stores.get(&epoch) {
            return store.clone();
        }
        let path = self.data_dir.join(format!("epoch-{epoch}"));
        let store = Arc::new(EpochStore::open_for_test(&path));
        stores.insert(epoch, store.clone());

        while stores.len() > self.max_epochs.max(1) {
            if let Some((&oldest_epoch, _)) = stores.first_key_value() {
                stores.remove(&oldest_epoch);
                let path = self.data_dir.join(format!("epoch-{oldest_epoch}"));
                if path.exists() {
                    let _ = std::fs::remove_dir_all(&path);
                }
            }
        }

        store
    }

    /// Look up an existing epoch store (read-only, does not create).
    pub fn get_epoch(&self, epoch: u64) -> Option<Arc<EpochStore>> {
        self.stores.read().get(&epoch).cloned()
    }

    /// Get the current (latest) epoch store, if any.
    pub fn current_epoch_store(&self) -> Option<Arc<EpochStore>> {
        self.stores.read().values().last().cloned()
    }

    /// Returns info about all available epochs as `(epoch, first_round, last_round)`.
    pub fn list_epochs(&self) -> Vec<(u64, u32, u32)> {
        let stores = self.stores.read();
        stores
            .iter()
            .map(|(&epoch, store)| {
                let first = store.get_first_round().max(1);
                let last = store.get_last_round();
                (epoch, first, last)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn open_test_store(dir: &Path) -> EpochStore {
        EpochStore::open_for_test(dir)
    }

    fn sample_block(round: u32, author: u8) -> StoredBlock {
        StoredBlock {
            round,
            author,
            digest: [round as u8; 32],
            timestamp_ms: round as u64 * 1000,
            ancestors: vec![(round.saturating_sub(1), 0)],
            acknowledgments: vec![(round.saturating_sub(1), 0, [0u8; 32])],
        }
    }

    fn sample_leader(leader_round: u32, status: u8) -> StoredLeader {
        StoredLeader {
            wave: leader_round / 2,
            leader_round,
            leader_authority: 0,
            status,
            block_digest: if status == 0 {
                Some([leader_round as u8; 32])
            } else {
                None
            },
        }
    }

    // --- EpochStore tests ---

    #[tokio::test]
    async fn block_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_test_store(dir.path());
        let block = sample_block(10, 1);
        store.insert_block(&block);

        let blocks = store.get_blocks_in_range(10, 10);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].round, 10);
        assert_eq!(blocks[0].author, 1);
        assert_eq!(blocks[0].digest, [10u8; 32]);
        assert_eq!(blocks[0].timestamp_ms, 10_000);
        assert_eq!(blocks[0].ancestors, vec![(9, 0)]);
        assert_eq!(blocks[0].acknowledgments, vec![(9, 0, [0u8; 32])]);
    }

    #[tokio::test]
    async fn leader_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_test_store(dir.path());
        let leader = sample_leader(6, 0);
        store.insert_leader(&leader);

        let leaders = store.get_leaders_in_range(6, 6);
        assert_eq!(leaders.len(), 1);
        assert_eq!(leaders[0].wave, 3);
        assert_eq!(leaders[0].leader_round, 6);
        assert_eq!(leaders[0].leader_authority, 0);
        assert_eq!(leaders[0].status, 0);
        assert_eq!(leaders[0].block_digest, Some([6u8; 32]));
    }

    #[tokio::test]
    async fn blocks_range_query() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_test_store(dir.path());
        for round in [5, 10, 15, 20] {
            store.insert_block(&sample_block(round, 0));
        }

        let blocks = store.get_blocks_in_range(8, 16);
        let rounds: Vec<u32> = blocks.iter().map(|b| b.round).collect();
        assert_eq!(rounds, vec![10, 15]);
    }

    #[tokio::test]
    async fn leaders_range_query() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_test_store(dir.path());
        for round in [1, 4, 7, 10] {
            store.insert_leader(&sample_leader(round, 0));
        }

        let leaders = store.get_leaders_in_range(3, 8);
        let rounds: Vec<u32> = leaders.iter().map(|l| l.leader_round).collect();
        assert_eq!(rounds, vec![4, 7]);
    }

    #[tokio::test]
    async fn committee_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_test_store(dir.path());
        let committee = StoredCommittee {
            epoch: 5,
            total_stake: 1000,
            quorum_threshold: 667,
            validators: vec![
                StoredValidator {
                    index: 0,
                    hostname: "node-0".to_string(),
                    stake: 500,
                },
                StoredValidator {
                    index: 1,
                    hostname: "node-1".to_string(),
                    stake: 500,
                },
            ],
        };
        store.set_committee(&committee);

        let loaded = store.get_committee().unwrap();
        assert_eq!(loaded.epoch, 5);
        assert_eq!(loaded.total_stake, 1000);
        assert_eq!(loaded.quorum_threshold, 667);
        assert_eq!(loaded.validators.len(), 2);
        assert_eq!(loaded.validators[0].hostname, "node-0");
        assert_eq!(loaded.validators[1].stake, 500);
    }

    #[tokio::test]
    async fn status_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_test_store(dir.path());
        let status = StoredStatus {
            highest_accepted_round: 100,
            last_commit_index: 50,
            last_commit_round: 98,
            num_authorities: 4,
        };
        store.set_status(&status);

        let loaded = store.get_status().unwrap();
        assert_eq!(loaded.highest_accepted_round, 100);
        assert_eq!(loaded.last_commit_index, 50);
        assert_eq!(loaded.last_commit_round, 98);
        assert_eq!(loaded.num_authorities, 4);
    }

    #[tokio::test]
    async fn last_round_default_zero() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_test_store(dir.path());
        assert_eq!(store.get_last_round(), 0);
    }

    #[tokio::test]
    async fn last_round_persists() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_test_store(dir.path());
        store.set_last_round(42);
        assert_eq!(store.get_last_round(), 42);
    }

    // --- StorageManager tests ---

    #[tokio::test]
    async fn get_or_create_epoch_creates() {
        let dir = tempfile::tempdir().unwrap();
        let manager = StorageManager::new(dir.path().to_path_buf(), 5);
        let _store = manager.get_or_create_epoch_for_test(1);
        assert!(manager.current_epoch_store().is_some());
    }

    #[tokio::test]
    async fn get_epoch_returns_none_for_missing() {
        let dir = tempfile::tempdir().unwrap();
        let manager = StorageManager::new(dir.path().to_path_buf(), 5);
        assert!(manager.get_epoch(99).is_none());
    }

    #[tokio::test]
    async fn epoch_pruning() {
        let dir = tempfile::tempdir().unwrap();
        let manager = StorageManager::new(dir.path().to_path_buf(), 2);
        manager.get_or_create_epoch_for_test(1);
        manager.get_or_create_epoch_for_test(2);
        manager.get_or_create_epoch_for_test(3);

        assert!(manager.get_epoch(1).is_none(), "epoch 1 should be pruned");
        assert!(manager.get_epoch(2).is_some());
        assert!(manager.get_epoch(3).is_some());
    }

    #[tokio::test]
    async fn list_epochs() {
        let dir = tempfile::tempdir().unwrap();
        let manager = StorageManager::new(dir.path().to_path_buf(), 5);
        let store1 = manager.get_or_create_epoch_for_test(1);
        store1.set_last_round(10);
        let store2 = manager.get_or_create_epoch_for_test(2);
        store2.set_last_round(20);

        let epochs = manager.list_epochs();
        assert_eq!(epochs.len(), 2);
        assert_eq!(epochs[0], (1, 1, 10));
        assert_eq!(epochs[1], (2, 1, 20));
    }
}
