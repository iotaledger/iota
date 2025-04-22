// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::PathBuf,
};

use iota_framework::SystemPackage;
use iota_types::{
    BRIDGE_PACKAGE_ID, IOTA_FRAMEWORK_PACKAGE_ID, IOTA_SYSTEM_PACKAGE_ID, MOVE_STDLIB_PACKAGE_ID,
    STARDUST_PACKAGE_ID, base_types::ObjectID,
};
use serde::{Deserialize, Serialize};

pub type SnapshotManifest = BTreeMap<u64, SingleSnapshot>;

#[derive(Serialize, Deserialize)]
pub struct SingleSnapshot {
    /// Git revision that this snapshot is taken on.
    git_revision: String,
    /// List of file names (also identical to object ID) of the bytecode package
    /// files.
    package_ids: Vec<ObjectID>,
}

impl SingleSnapshot {
    pub fn git_revision(&self) -> &str {
        &self.git_revision
    }
    pub fn package_ids(&self) -> &[ObjectID] {
        &self.package_ids
    }
}

const SYSTEM_PACKAGE_PUBLISH_ORDER: &[ObjectID] = &[
    MOVE_STDLIB_PACKAGE_ID,
    IOTA_FRAMEWORK_PACKAGE_ID,
    IOTA_SYSTEM_PACKAGE_ID,
    BRIDGE_PACKAGE_ID,
    STARDUST_PACKAGE_ID,
];

pub fn load_bytecode_snapshot_manifest() -> SnapshotManifest {
    let Ok(bytes) = fs::read(manifest_path()) else {
        return SnapshotManifest::default();
    };
    serde_json::from_slice::<SnapshotManifest>(&bytes)
        .expect("Could not deserialize SnapshotManifest")
}

pub fn update_bytecode_snapshot_manifest(git_revision: &str, version: u64, files: Vec<ObjectID>) {
    let mut snapshot = load_bytecode_snapshot_manifest();

    snapshot.insert(
        version,
        SingleSnapshot {
            git_revision: git_revision.to_string(),
            package_ids: files,
        },
    );

    let json =
        serde_json::to_string_pretty(&snapshot).expect("Could not serialize SnapshotManifest");
    fs::write(manifest_path(), json).expect("Could not update manifest file");
}

pub fn load_bytecode_snapshot(protocol_version: u64) -> anyhow::Result<Vec<SystemPackage>> {
    let snapshot_path = snapshot_path_for_version(protocol_version)?;
    let mut snapshots: BTreeMap<ObjectID, SystemPackage> = fs::read_dir(&snapshot_path)?
        .flatten()
        .map(|entry| {
            let file_name = entry.file_name().to_str().unwrap().to_string();
            let mut file = fs::File::open(snapshot_path.clone().join(file_name))?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;
            let package: SystemPackage = bcs::from_bytes(&buffer)?;
            Ok((*package.id(), package))
        })
        .collect::<anyhow::Result<_>>()?;

    // system packages need to be restored in a specific order
    assert!(snapshots.len() <= SYSTEM_PACKAGE_PUBLISH_ORDER.len());
    let mut snapshot_objects = Vec::new();
    for package_id in SYSTEM_PACKAGE_PUBLISH_ORDER {
        if let Some(object) = snapshots.remove(package_id) {
            snapshot_objects.push(object);
        }
    }
    Ok(snapshot_objects)
}

fn manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("manifest.json")
}

/// Given a protocol version:
/// * The path to the snapshot directory for that version is returned, if it
///   exists.
/// * If the version is greater than the latest snapshot version, then
///   `Ok(None)` is returned.
/// * If the version does not exist, but there are snapshots present with
///   versions greater than `version`, then the smallest snapshot number greater
///   than `version` is returned.
fn snapshot_path_for_version(version: u64) -> anyhow::Result<PathBuf> {
    let snapshot_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bytecode_snapshot");
    let mut snapshots = BTreeSet::new();

    for entry in fs::read_dir(&snapshot_dir)? {
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

    snapshots
        .range(version..)
        .next()
        .map(|v| snapshot_dir.join(v.to_string()))
        .ok_or_else(|| anyhow::anyhow!("No snapshot found for version {}", version))
}
