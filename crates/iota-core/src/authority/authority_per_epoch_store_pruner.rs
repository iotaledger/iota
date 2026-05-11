// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fs, path::PathBuf, time::Duration};

use itertools::Itertools;
use tracing::{info, warn};
use typed_store::rocks::safe_drop_db;

use crate::authority::authority_per_epoch_store::EPOCH_DB_PREFIX;

/// The `AuthorityPerEpochStorePruner` manages the pruning process for Authority
/// Store databases on a per-epoch basis. It retains only the N most recent (by
/// epoch number). Pruning is triggered at node startup and after each epoch
/// transition.
pub struct AuthorityPerEpochStorePruner {
    parent_path: PathBuf,
    num_latest_epoch_dbs_to_retain: usize,
}

impl AuthorityPerEpochStorePruner {
    /// Creates a new epoch DB pruner and immediately prunes any stale epoch
    /// databases left over from a previous run (e.g. after a crash).
    pub async fn new(parent_path: PathBuf, num_latest_epoch_dbs_to_retain: usize) -> Self {
        let pruner = Self {
            parent_path,
            num_latest_epoch_dbs_to_retain,
        };
        // Prune on startup to clean up stale epoch DBs from prior runs.
        pruner.prune_old_epoch_dbs().await;
        pruner
    }

    /// Prunes old epoch databases, retaining only the configured number of
    /// most recent ones. Should be called after each epoch transition.
    pub async fn prune_old_epoch_dbs(&self) {
        if self.num_latest_epoch_dbs_to_retain == 0
            || self.num_latest_epoch_dbs_to_retain == usize::MAX
        {
            return;
        }
        match Self::prune_old_directories(&self.parent_path, self.num_latest_epoch_dbs_to_retain)
            .await
        {
            Ok(pruned_count) => {
                if pruned_count > 0 {
                    info!("Pruned {} old epoch databases", pruned_count);
                }
            }
            Err(err) => warn!("Error while removing old epoch databases: {:?}", err),
        }
    }

    /// Prunes old epoch directories from the specified parent path, retaining
    /// only the latest specified number of epoch databases. This function
    /// identifies epoch directories, sorts them, and deletes the older
    /// ones. Returns the number of directories pruned or an error if the
    /// pruning process encounters an issue.
    async fn prune_old_directories(
        parent_path: &PathBuf,
        num_latest_epoch_dbs_to_retain: usize,
    ) -> Result<usize, anyhow::Error> {
        let mut candidates = vec![];
        let directories = fs::read_dir(parent_path)?.collect::<Result<Vec<_>, _>>()?;
        for directory in directories {
            let path = directory.path();
            if let Some(filename) = directory.file_name().to_str() {
                if let Ok(epoch) = filename.split_at(EPOCH_DB_PREFIX.len()).1.parse::<u64>() {
                    candidates.push((epoch, path));
                }
            }
        }
        let mut pruned = 0;
        let mut gc_tasks = vec![];
        if num_latest_epoch_dbs_to_retain < candidates.len() {
            let to_prune = candidates.len() - num_latest_epoch_dbs_to_retain;
            for (_, path) in candidates.into_iter().sorted().take(to_prune) {
                info!("Dropping epoch directory {:?}", path);
                pruned += 1;
                gc_tasks.push(safe_drop_db(
                    path.join("recovery_log"),
                    Duration::from_secs(30),
                ));
                gc_tasks.push(safe_drop_db(path, Duration::from_secs(30)));
            }
        }
        futures::future::join_all(gc_tasks)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
        Ok(pruned)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::authority::authority_per_epoch_store_pruner::AuthorityPerEpochStorePruner;

    #[tokio::test]
    async fn test_basic_epoch_pruner() {
        let tmp_dir = iota_common::tempdir();
        let directories: Vec<_> = vec!["epoch_0", "epoch_1", "epoch_3", "epoch_4"]
            .into_iter()
            .map(|name| tmp_dir.path().join(name))
            .collect();
        for directory in &directories {
            fs::create_dir(directory).expect("failed to create directory");
        }

        let pruned =
            AuthorityPerEpochStorePruner::prune_old_directories(&tmp_dir.path().to_path_buf(), 2)
                .await
                .unwrap();
        assert_eq!(pruned, 2);
        assert_eq!(
            directories
                .into_iter()
                .map(|f| fs::metadata(f).is_ok())
                .collect::<Vec<_>>(),
            vec![false, false, true, true]
        );
    }
}
