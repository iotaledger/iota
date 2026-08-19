// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, fs, path::PathBuf};

use iota_sdk_types::ObjectId;
use serde::{Deserialize, Serialize};

pub type SnapshotManifest = BTreeMap<u64, Snapshot>;

/// Encapsulation of an entry in the manifest file corresponding to a single
/// version of the system packages.
// Note: the [Snapshot] and [SnapshotPackage] types are similar to the
// [iota_framework::{SystemPackageMetadata, SystemPackage}] types, and also to the
// [iota_package_management::{SystemPackagesVersion, SystemPackage}] types.
// They are sort of a stepping stone from one to the other - the [iota_framework] types contain
// additional information about the compiled bytecode of the package, while the
// [iota_package_management] types do not contain information about the object IDs of the packages.
//
// These types serve as a kind of stepping stone; they are constructed from the [iota_framework]
// types and serialized in the manifest, and then the build script of [iota_package_management]
// reads them from the manifest file and encodes them in its version table. A little information is
// dropped in each of these steps.
#[derive(Serialize, Deserialize)]
pub struct Snapshot {
    /// Git revision that this snapshot is taken on.
    pub git_revision: String,
    /// List of system packages in this version.
    pub packages: Vec<SnapshotPackage>,
}

/// Entry in the manifest file corresponding to a specific version of a specific
/// system package.
#[derive(Serialize, Deserialize)]
pub struct SnapshotPackage {
    /// Name of the package (e.g. "MoveStdLib").
    pub name: String,
    /// Path to the package in the monorepo (e.g.
    /// "crates/iota-framework/packages/move-stdlib").
    pub path: String,
    /// Object ID of the published package.
    pub id: ObjectId,
}

impl Snapshot {
    pub fn package_ids(&self) -> impl Iterator<Item = ObjectId> + '_ {
        self.packages.iter().map(|p| p.id)
    }
}

pub fn load_bytecode_snapshot_manifest() -> SnapshotManifest {
    let Ok(bytes) = fs::read(manifest_path()) else {
        return SnapshotManifest::default();
    };
    serde_json::from_slice::<SnapshotManifest>(&bytes)
        .expect("Could not deserialize SnapshotManifest")
}

pub fn update_bytecode_snapshot_manifest(
    git_revision: &str,
    version: u64,
    files: Vec<SnapshotPackage>,
) {
    let mut snapshot = load_bytecode_snapshot_manifest();

    snapshot.insert(
        version,
        Snapshot {
            git_revision: git_revision.to_string(),
            packages: files,
        },
    );

    let json =
        serde_json::to_string_pretty(&snapshot).expect("Could not serialize SnapshotManifest");
    fs::write(manifest_path(), json).expect("Could not update manifest file");
}

pub fn manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("manifest.json")
}
