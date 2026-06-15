// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use iota_types::{
    committee::{Committee, EpochId},
    error::{IotaError, IotaResult},
};
use parking_lot::RwLock;
use typed_store::{
    DBMapUtils, Map,
    rocks::{DBMap, DBOptions, MetricConf, default_db_options},
    rocksdb::Options,
};

pub struct CommitteeStore {
    tables: CommitteeStoreTables,
    cache: RwLock<HashMap<EpochId, Arc<Committee>>>,
}

#[derive(DBMapUtils)]
pub struct CommitteeStoreTables {
    /// Map from each epoch ID to the committee information.
    #[default_options_override_fn = "committee_table_default_config"]
    committee_map: DBMap<EpochId, Committee>,
}

// These functions are used to initialize the DB tables
fn committee_table_default_config() -> DBOptions {
    default_db_options().optimize_for_point_lookup(64)
}

impl CommitteeStore {
    /// Open the on-disk tables at `path` into an empty-cache store, without
    /// touching the genesis committee.
    fn open_tables(path: PathBuf, db_options: Option<Options>) -> Self {
        let tables = CommitteeStoreTables::open_tables_read_write(
            path,
            MetricConf::new("committee"),
            db_options,
            None,
        );
        Self {
            tables,
            cache: RwLock::new(HashMap::new()),
        }
    }

    pub fn new(path: PathBuf, genesis_committee: &Committee, db_options: Option<Options>) -> Self {
        let store = Self::open_tables(path, db_options);
        if store
            .database_is_empty()
            .expect("CommitteeStore initialization failed")
        {
            store
                .init_genesis_committee(genesis_committee.clone())
                .expect("Init genesis committee data must not fail");
        }
        store
    }

    pub fn new_for_testing(genesis_committee: &Committee) -> Self {
        let path = iota_common::tempdir().keep();
        Self::new(path, genesis_committee, None)
    }

    /// Open an existing committee store whose genesis committee is already
    /// persisted (e.g. a restored or synced node's store). Unlike [`Self::new`]
    /// it takes no genesis committee — it errors if the store has none, rather
    /// than initializing one.
    pub fn open(path: PathBuf, db_options: Option<Options>) -> IotaResult<Self> {
        let store = Self::open_tables(path, db_options);
        if store.database_is_empty()? {
            return Err(IotaError::Storage(
                "committee store has no genesis committee".to_string(),
            ));
        }
        Ok(store)
    }

    pub fn init_genesis_committee(&self, genesis_committee: Committee) -> IotaResult {
        assert_eq!(genesis_committee.epoch, 0);
        self.tables.committee_map.insert(&0, &genesis_committee)?;
        self.cache.write().insert(0, Arc::new(genesis_committee));
        Ok(())
    }

    pub fn insert_new_committee(&self, new_committee: &Committee) -> IotaResult {
        if let Some(old_committee) = self.get_committee(&new_committee.epoch)? {
            // If somehow we already have this committee in the store, they must be the
            // same.
            assert_eq!(&*old_committee, new_committee);
        } else {
            self.tables
                .committee_map
                .insert(&new_committee.epoch, new_committee)?;
            self.cache
                .write()
                .insert(new_committee.epoch, Arc::new(new_committee.clone()));
        }
        Ok(())
    }

    pub fn get_committee(&self, epoch_id: &EpochId) -> IotaResult<Option<Arc<Committee>>> {
        if let Some(committee) = self.cache.read().get(epoch_id) {
            return Ok(Some(committee.clone()));
        }
        let committee = self.tables.committee_map.get(epoch_id)?;
        let committee = committee.map(Arc::new);
        if let Some(committee) = committee.as_ref() {
            self.cache.write().insert(*epoch_id, committee.clone());
        }
        Ok(committee)
    }

    // todo - make use of cache or remove this method
    pub fn get_latest_committee(&self) -> IotaResult<Committee> {
        Ok(self
            .tables
            .committee_map
            .reversed_safe_iter_with_bounds(None, None)?
            .next()
            .transpose()?
            // unwrap safe because we guarantee there is at least a genesis epoch
            // when initializing the store.
            .unwrap()
            .1)
    }
    /// Return the committee specified by `epoch`. If `epoch` is `None`, return
    /// the latest committee.
    // todo - make use of cache or remove this method
    pub fn get_or_latest_committee(&self, epoch: Option<EpochId>) -> IotaResult<Committee> {
        Ok(match epoch {
            Some(epoch) => self
                .get_committee(&epoch)?
                .ok_or(IotaError::MissingCommitteeAtEpoch(epoch))
                .map(|c| Committee::clone(&*c))?,
            None => self.get_latest_committee()?,
        })
    }

    pub fn checkpoint_db(&self, path: &Path) -> IotaResult {
        self.tables
            .committee_map
            .checkpoint_db(path)
            .map_err(Into::into)
    }

    fn database_is_empty(&self) -> IotaResult<bool> {
        Ok(self
            .tables
            .committee_map
            .safe_iter()
            .next()
            .transpose()?
            .is_none())
    }
}

#[cfg(test)]
mod tests {
    use iota_types::committee::Committee;

    use super::*;

    #[tokio::test]
    async fn open_reads_existing_genesis_and_rejects_empty() {
        let dir = iota_common::tempdir();
        let path = dir.path().to_path_buf();
        let (genesis_committee, _) = Committee::new_simple_test_committee();

        // A fresh directory has no genesis committee yet.
        assert!(CommitteeStore::open(path.clone(), None).is_err());

        // `new` initializes the genesis committee; `open` then reads it back
        // without being handed one.
        {
            let _store = CommitteeStore::new(path.clone(), &genesis_committee, None);
        }
        let opened = CommitteeStore::open(path, None).expect("store has a genesis committee");
        assert_eq!(
            *opened.get_committee(&0).unwrap().unwrap(),
            genesis_committee,
        );
    }
}
