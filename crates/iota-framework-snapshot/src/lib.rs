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
pub use iota_framework_snapshot_manifest::*;
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
        .ok_or_else(|| anyhow::anyhow!("No snapshot found for version {version}"))
}
