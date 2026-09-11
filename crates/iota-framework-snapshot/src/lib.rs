// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
};

use iota_framework::SystemPackage;
pub use iota_framework_snapshot_manifest::*;
use iota_protocol_config::ProtocolVersion;
use iota_sdk_types::ObjectId;

const SYSTEM_PACKAGE_PUBLISH_ORDER: &[ObjectId] = &[
    ObjectId::STD,
    ObjectId::FRAMEWORK,
    ObjectId::SYSTEM,
    ObjectId::STARDUST,
];

/// Returns the list of system packages in the order they should be published.
/// If the protocol version is < 9 then include also the bridge package.
pub fn get_system_package_publish_order(protocol_version: u64) -> Vec<ObjectId> {
    let mut publish_order = SYSTEM_PACKAGE_PUBLISH_ORDER.to_vec();
    if protocol_version < 9 {
        publish_order.insert(3, ObjectId::GENESIS_BRIDGE);
    }
    publish_order
}

pub fn load_bytecode_snapshot(protocol_version: u64) -> anyhow::Result<Vec<SystemPackage>> {
    let snapshot_path = snapshot_path_for_version(protocol_version)?;
    let mut snapshots: BTreeMap<ObjectId, SystemPackage> = fs::read_dir(&snapshot_path)?
        .flatten()
        .map(|entry| {
            let file_name = entry.file_name().to_str().unwrap().to_string();
            let mut file = fs::File::open(snapshot_path.clone().join(file_name))?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;
            let package: SystemPackage = bcs::from_bytes(&buffer)?;
            Ok((package.id, package))
        })
        .collect::<anyhow::Result<_>>()?;

    // system packages need to be restored in a specific order
    let snapshots_publish_order = get_system_package_publish_order(protocol_version);
    assert!(snapshots.len() <= snapshots_publish_order.len());
    let mut snapshot_objects = Vec::new();
    for package_id in &snapshots_publish_order {
        if let Some(object) = snapshots.remove(package_id) {
            snapshot_objects.push(object);
        }
    }
    Ok(snapshot_objects)
}

/// Returns the path of the snapshot directory holding the framework in effect
/// at `version`, which is the newest snapshot taken at or before it. A
/// protocol version that changed nothing in the framework has no snapshot of
/// its own and resolves to the last one taken before it.
///
/// Returns an error if `version` is the max protocol version supported by this
/// build and no snapshot has been taken for it yet, so that callers fall back
/// to the framework compiled into this build rather than to an older snapshot.
/// An error is also returned if `version` predates every snapshot.
fn snapshot_path_for_version(version: u64) -> anyhow::Result<PathBuf> {
    let snapshot_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bytecode_snapshot");
    let snapshots = read_snapshot_versions(&snapshot_dir)?;
    let selected = select_snapshot_version(version, ProtocolVersion::MAX.as_u64(), &snapshots)?;

    Ok(snapshot_dir.join(selected.to_string()))
}

/// Returns the protocol versions that `snapshot_dir` holds a snapshot for.
fn read_snapshot_versions(snapshot_dir: &Path) -> anyhow::Result<BTreeSet<u64>> {
    let mut snapshots = BTreeSet::new();

    for entry in fs::read_dir(snapshot_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(snapshot_number) = path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.parse::<u64>().ok())
            {
                snapshots.insert(snapshot_number);
            }
        }
    }

    Ok(snapshots)
}

/// Picks the newest snapshot taken at or before `version` out of `snapshots`.
///
/// `max_protocol_version` never resolves to an earlier snapshot: while it has
/// no snapshot of its own, the framework in effect at that version is the one
/// compiled into this build, which is not on disk.
fn select_snapshot_version(
    version: u64,
    max_protocol_version: u64,
    snapshots: &BTreeSet<u64>,
) -> anyhow::Result<u64> {
    if version == max_protocol_version && !snapshots.contains(&version) {
        anyhow::bail!("No snapshot found for version {version}");
    }

    snapshots
        .range(..=version)
        .next_back()
        .copied()
        .ok_or_else(|| anyhow::anyhow!("No snapshot found for version {version}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_snapshot_of_the_requested_version() {
        let snapshots = BTreeSet::from([1, 2, 3]);
        assert_eq!(select_snapshot_version(2, 3, &snapshots).unwrap(), 2);
    }

    #[test]
    fn select_snapshot_preceding_a_version_without_one() {
        let snapshots = BTreeSet::from([33, 35]);
        assert_eq!(select_snapshot_version(34, 36, &snapshots).unwrap(), 33);
    }

    #[test]
    fn select_snapshot_of_the_requested_version_past_a_gap() {
        let snapshots = BTreeSet::from([33, 35]);
        assert_eq!(select_snapshot_version(35, 36, &snapshots).unwrap(), 35);
    }

    #[test]
    fn select_no_snapshot_for_the_max_version_without_one() {
        let snapshots = BTreeSet::from([33, 35]);
        assert!(select_snapshot_version(36, 36, &snapshots).is_err());
    }

    #[test]
    fn select_snapshot_of_the_max_version_when_it_has_one() {
        let snapshots = BTreeSet::from([33, 34, 35]);
        assert_eq!(select_snapshot_version(35, 35, &snapshots).unwrap(), 35);
    }

    #[test]
    fn select_no_snapshot_for_a_version_preceding_all_of_them() {
        let snapshots = BTreeSet::from([10, 11]);
        assert!(select_snapshot_version(5, 12, &snapshots).is_err());
    }
}
