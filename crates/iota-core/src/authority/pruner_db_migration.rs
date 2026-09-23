// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! One-time cleanup of the `pruner` database left behind by the removed
//! objects compaction filter.
//!
//! A node running with the compaction filter recorded, per object, the highest
//! version the pruner had superseded, and left it to compaction to drop those
//! rows from the objects table. Without the filter nothing drops them, so the
//! rows are deleted here with the same range deletes the pruner writes on its
//! own, and the `pruner` database is then removed.

// TODO(#12968): remove this module once a release containing it has shipped.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use iota_sdk_types::{ObjectId, Version};
use iota_types::storage::ObjectKey;
use tracing::info;
use typed_store::{
    DBMapUtils,
    metrics::SamplingInterval,
    rocks::{DBMap, MetricConf},
    traits::Map,
};

use crate::authority::authority_store_tables::AuthorityPerpetualTables;

/// Number of tombstone entries drained per write batch.
const DRAIN_CHUNK_SIZE: usize = 10_000;

#[derive(DBMapUtils)]
struct LeftoverPrunerTables {
    /// Highest object version the pruner superseded, per object.
    object_tombstones: DBMap<ObjectId, Version>,
}

impl LeftoverPrunerTables {
    fn path(parent_path: &Path) -> PathBuf {
        parent_path.join("pruner")
    }

    fn open(parent_path: &Path) -> Self {
        Self::open_tables_read_write(
            Self::path(parent_path),
            MetricConf::new("pruner")
                .with_sampling(SamplingInterval::new(Duration::from_secs(60), 0)),
            None,
            None,
        )
    }
}

/// Deletes the object versions recorded in a leftover `pruner` database and
/// removes that database. Returns the number of objects whose versions were
/// deleted.
///
/// Does nothing when there is no `pruner` database under `parent_path`, which
/// is the case for every node that never ran with the compaction filter.
pub(crate) fn drain_leftover_object_tombstones(
    parent_path: &Path,
    perpetual_tables: &AuthorityPerpetualTables,
) -> anyhow::Result<usize> {
    let path = LeftoverPrunerTables::path(parent_path);
    // Checked before opening, which would create the database.
    if !path.exists() {
        return Ok(0);
    }
    info!(
        "draining the leftover object tombstones in {}",
        path.display()
    );

    let leftover = LeftoverPrunerTables::open(parent_path);
    let mut drained = 0;
    let mut deleted = 0;
    let mut resume_from = None;
    loop {
        let chunk = leftover
            .object_tombstones
            .safe_iter_with_bounds(resume_from, None)
            .take(DRAIN_CHUNK_SIZE)
            .collect::<Result<Vec<_>, _>>()?;
        if chunk.is_empty() {
            break;
        }

        let mut batch = perpetual_tables.objects.batch();
        for (object_id, gc_version) in &chunk {
            let (from, to) = (
                ObjectKey(*object_id, Version::MIN_VALID_INCL),
                ObjectKey(*object_id, *gc_version + 1),
            );
            // An entry outlives the versions it covers: the table was only
            // ever appended to, so most entries name versions a compaction
            // already dropped. A range delete over them would write a
            // tombstone that hides nothing and still has to be read past
            // until the next compaction. Probing first keeps the writes
            // proportional to what is actually still there, and costs little
            // because both tables are ordered by object id, so the probes
            // walk the objects table forwards.
            if perpetual_tables
                .objects
                .safe_iter_with_bounds(Some(from), Some(to))
                .next()
                .transpose()?
                .is_none()
            {
                continue;
            }
            batch.schedule_delete_range(&perpetual_tables.objects, &from, &to)?;
            deleted += 1;
        }
        batch.write()?;

        // The entries are forgotten only after their object versions are gone,
        // so a drain interrupted here replays deletes that are already no-ops.
        leftover
            .object_tombstones
            .multi_remove(chunk.iter().map(|(object_id, _)| *object_id))?;

        // Resume past the entries just removed rather than scanning over them
        // again on the next round.
        resume_from = chunk.last().map(|(object_id, _)| *object_id);
        drained += chunk.len();
        if chunk.len() == DRAIN_CHUNK_SIZE {
            info!("drained {drained} leftover object tombstones so far");
        }
    }

    drop(leftover);
    std::fs::remove_dir_all(&path)?;
    info!(
        "drained {drained} leftover object tombstones, deleted the versions of {deleted} objects \
         and removed {}",
        path.display()
    );
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use iota_types::object::Object;

    use super::*;
    use crate::authority::authority_store_types::get_store_object;

    fn insert_object_versions(
        perpetual_tables: &AuthorityPerpetualTables,
        object_id: ObjectId,
        versions: impl IntoIterator<Item = u64>,
    ) {
        let mut batch = perpetual_tables.objects.batch();
        for version in versions {
            batch
                .insert_batch(
                    &perpetual_tables.objects,
                    [(
                        ObjectKey(object_id, Version::from_u64(version)),
                        get_store_object(Object::immutable_with_id_for_testing(object_id), None),
                    )],
                )
                .unwrap();
        }
        batch.write().unwrap();
    }

    fn object_versions(
        perpetual_tables: &AuthorityPerpetualTables,
        object_id: ObjectId,
    ) -> Vec<u64> {
        perpetual_tables
            .objects
            .safe_iter()
            .map(|entry| entry.unwrap().0)
            .filter(|key| key.0 == object_id)
            .map(|key| key.1.as_u64())
            .collect()
    }

    fn delete_object_versions(
        perpetual_tables: &AuthorityPerpetualTables,
        object_id: ObjectId,
        up_to_incl: u64,
    ) {
        let mut batch = perpetual_tables.objects.batch();
        batch
            .schedule_delete_range(
                &perpetual_tables.objects,
                &ObjectKey(object_id, Version::MIN_VALID_INCL),
                &ObjectKey(object_id, Version::from_u64(up_to_incl) + 1),
            )
            .unwrap();
        batch.write().unwrap();
    }

    #[tokio::test]
    async fn drains_the_tombstoned_versions_and_removes_the_pruner_db() {
        let tmp_dir = iota_common::tempdir();
        let path = tmp_dir.path();
        let object_id = ObjectId::random();

        {
            let perpetual_tables = AuthorityPerpetualTables::open(path, None);
            insert_object_versions(&perpetual_tables, object_id, 1..=5);
            let leftover = LeftoverPrunerTables::open(path);
            leftover
                .object_tombstones
                .insert(&object_id, &Version::from_u64(3))
                .unwrap();
        }

        let perpetual_tables = AuthorityPerpetualTables::open(path, None);

        assert_eq!(object_versions(&perpetual_tables, object_id), vec![4, 5]);
        assert!(!LeftoverPrunerTables::path(path).exists());
    }

    #[tokio::test]
    async fn skips_entries_whose_versions_are_already_gone() {
        let tmp_dir = iota_common::tempdir();
        let path = tmp_dir.path();
        let (compacted_away, still_live) = (ObjectId::ZERO, ObjectId::ZERO.next_lexicographical());

        let perpetual_tables = AuthorityPerpetualTables::open(path, None);
        // Only versions above the watermark are left for `still_live`, and
        // nothing at all is left for `compacted_away`, so neither entry has
        // anything to delete.
        insert_object_versions(&perpetual_tables, still_live, 4..=5);
        let leftover = LeftoverPrunerTables::open(path);
        for object_id in [compacted_away, still_live] {
            leftover
                .object_tombstones
                .insert(&object_id, &Version::from_u64(3))
                .unwrap();
        }
        drop(leftover);

        let deleted = drain_leftover_object_tombstones(path, &perpetual_tables).unwrap();

        assert_eq!(deleted, 0);
        assert_eq!(object_versions(&perpetual_tables, still_live), vec![4, 5]);
        assert!(!LeftoverPrunerTables::path(path).exists());
    }

    #[tokio::test]
    async fn does_not_create_a_pruner_db_when_none_exists() {
        let tmp_dir = iota_common::tempdir();
        let path = tmp_dir.path();

        let _perpetual_tables = AuthorityPerpetualTables::open(path, None);

        assert!(!LeftoverPrunerTables::path(path).exists());
    }

    #[tokio::test]
    async fn removes_an_empty_pruner_db() {
        let tmp_dir = iota_common::tempdir();
        let path = tmp_dir.path();
        drop(LeftoverPrunerTables::open(path));
        assert!(LeftoverPrunerTables::path(path).exists());

        let _perpetual_tables = AuthorityPerpetualTables::open(path, None);

        assert!(!LeftoverPrunerTables::path(path).exists());
    }

    #[tokio::test]
    async fn a_replayed_drain_converges() {
        let tmp_dir = iota_common::tempdir();
        let path = tmp_dir.path();
        let (first, second) = (ObjectId::ZERO, ObjectId::ZERO.next_lexicographical());

        {
            let perpetual_tables = AuthorityPerpetualTables::open(path, None);
            insert_object_versions(&perpetual_tables, first, 1..=5);
            insert_object_versions(&perpetual_tables, second, 1..=5);
            let leftover = LeftoverPrunerTables::open(path);
            for object_id in [first, second] {
                leftover
                    .object_tombstones
                    .insert(&object_id, &Version::from_u64(3))
                    .unwrap();
            }
            // A drain interrupted after the object rows of `first` were
            // deleted but before its tombstone entry was cleared: the entry
            // is replayed on the next open.
            delete_object_versions(&perpetual_tables, first, 3);
        }

        let perpetual_tables = AuthorityPerpetualTables::open(path, None);

        assert_eq!(object_versions(&perpetual_tables, first), vec![4, 5]);
        assert_eq!(object_versions(&perpetual_tables, second), vec![4, 5]);
        assert!(!LeftoverPrunerTables::path(path).exists());
    }
}
