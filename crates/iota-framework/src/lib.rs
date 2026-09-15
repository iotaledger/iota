// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{fmt::Formatter, sync::LazyLock};

use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{MovePackage, ObjectId, ObjectReference, TransactionDigest};
use iota_types::{
    execution_config_utils::to_binary_config,
    move_package::{MovePackageExt, max_package_size},
    object::{OBJECT_START_VERSION, Object},
    storage::ObjectStore,
};
use move_binary_format::{CompiledModule, compatibility::Compatibility, normalized};
use move_core_types::gas_algebra::InternalGas;
use serde::{Deserialize, Serialize};
use tracing::error;

/// Encapsulates a system package in the framework.
pub struct SystemPackageMetadata {
    /// The name of the package (e.g. "MoveStdLib").
    pub name: String,
    /// The path within the repo to the source (e.g.
    /// "crates/iota-framework/packages/move-stdlib").
    pub path: String,
    /// The compiled bytecode and object ID of the package.
    pub compiled: SystemPackage,
}

/// Encapsulates the chain-relevant data about a framework package (such as the
/// id or compiled bytecode).
#[derive(Clone, Serialize, PartialEq, Eq, Deserialize)]
pub struct SystemPackage {
    pub id: ObjectId,
    pub bytes: Vec<Vec<u8>>,
    pub dependencies: Vec<ObjectId>,
}

impl SystemPackageMetadata {
    pub fn new(
        name: impl ToString,
        path: impl ToString,
        id: ObjectId,
        raw_bytes: &'static [u8],
        dependencies: &[ObjectId],
    ) -> Self {
        SystemPackageMetadata {
            name: name.to_string(),
            path: path.to_string(),
            compiled: SystemPackage::new(id, raw_bytes, dependencies),
        }
    }
}

impl SystemPackage {
    pub fn new(id: ObjectId, raw_bytes: &'static [u8], dependencies: &[ObjectId]) -> Self {
        let bytes: Vec<Vec<u8>> = bcs::from_bytes(raw_bytes).unwrap();
        Self {
            id,
            bytes,
            dependencies: dependencies.to_vec(),
        }
    }

    pub fn modules(&self) -> Vec<CompiledModule> {
        self.bytes
            .iter()
            .map(|b| CompiledModule::deserialize_with_defaults(b).unwrap())
            .collect()
    }

    pub fn genesis_move_package(&self) -> MovePackage {
        MovePackage::new_system(
            OBJECT_START_VERSION,
            &self.modules(),
            self.dependencies.iter().copied(),
        )
    }

    pub fn genesis_object(&self) -> Object {
        Object::new_system_package(
            &self.modules(),
            OBJECT_START_VERSION,
            self.dependencies.to_vec(),
            TransactionDigest::GENESIS_MARKER,
        )
    }
}

impl std::fmt::Debug for SystemPackage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Object ID: {:?}", self.id)?;
        writeln!(f, "Size: {}", self.bytes.len())?;
        writeln!(f, "Dependencies: {:?}", self.dependencies)?;
        Ok(())
    }
}

macro_rules! define_system_package_metadata {
    ([$(($id:expr, $name: expr, $path:expr, $deps:expr)),* $(,)?]) => {{
        static PACKAGES: LazyLock<Vec<SystemPackageMetadata>> = LazyLock::new(|| {
            vec![
                $(SystemPackageMetadata::new(
                    $name,
                    concat!("crates/iota-framework/packages/", $path),
                    $id,
                    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/packages_compiled", "/", $path)),
                    &$deps,
                )),*
            ]
        });
        &PACKAGES
    }}
}

pub struct BuiltInFramework;
impl BuiltInFramework {
    pub fn iter_system_package_metadata() -> impl Iterator<Item = &'static SystemPackageMetadata> {
        // All system packages in the current build should be registered here, and this
        // is the only place we need to worry about if any of them changes.
        // TODO: Is it possible to derive dependencies from the bytecode instead of
        // manually specifying them?
        define_system_package_metadata!([
            (ObjectId::STD, "MoveStdlib", "move-stdlib", []),
            (
                ObjectId::FRAMEWORK,
                "Iota",
                "iota-framework",
                [ObjectId::STD]
            ),
            (
                ObjectId::SYSTEM,
                "IotaSystem",
                "iota-system",
                [ObjectId::STD, ObjectId::FRAMEWORK]
            ),
            (
                ObjectId::STARDUST,
                "Stardust",
                "stardust",
                [ObjectId::STD, ObjectId::FRAMEWORK]
            ),
        ])
        .iter()
    }

    pub fn all_package_ids() -> Vec<ObjectId> {
        Self::iter_system_packages().map(|p| p.id).collect()
    }

    pub fn get_package_by_id(id: &ObjectId) -> &'static SystemPackage {
        Self::iter_system_packages().find(|s| &s.id == id).unwrap()
    }

    pub fn iter_system_packages() -> impl Iterator<Item = &'static SystemPackage> {
        BuiltInFramework::iter_system_package_metadata().map(|m| &m.compiled)
    }

    pub fn genesis_move_packages() -> impl Iterator<Item = MovePackage> {
        Self::iter_system_packages().map(|package| package.genesis_move_package())
    }

    pub fn genesis_objects() -> impl Iterator<Item = Object> {
        Self::iter_system_packages().map(|package| package.genesis_object())
    }
}

pub const DEFAULT_FRAMEWORK_PATH: &str = env!("CARGO_MANIFEST_DIR");

pub fn legacy_test_cost() -> InternalGas {
    InternalGas::new(0)
}

/// Check whether the framework defined by `modules` is compatible with the
/// framework that is already on-chain (i.e. stored in `object_store`) at `id`.
///
/// - Panics if the object at `id` can be loaded but is not a package -- this is
///   an invariant violation.
/// - Returns the digest of the current framework (and version) if it is
///   equivalent to the new framework (indicates support for a protocol upgrade
///   without a framework upgrade).
/// - Returns the digest of the new framework (and version) if it is compatible
///   (indicates support for a protocol upgrade with a framework upgrade).
/// - Returns `None` if the current package at `id` cannot be loaded, or the
///   compatibility check fails (This is grounds not to upgrade).
/// - Returns `None` if the package does not exist on-chain yet and exceeds the
///   size a system package may occupy (This is grounds not to upgrade).
///
/// `protocol_config` must be the config of the epoch the upgrade would be
/// performed in, which is the one the resulting change epoch transaction
/// executes under.
pub async fn compare_system_package<S: ObjectStore>(
    object_store: &S,
    id: &ObjectId,
    modules: &[CompiledModule],
    dependencies: Vec<ObjectId>,
    protocol_config: &ProtocolConfig,
) -> Option<ObjectReference> {
    let binary_config = &to_binary_config(protocol_config);
    let cur_object = match object_store.try_get_object(id) {
        Ok(Some(cur_object)) => cur_object,

        Ok(None) => {
            // creating a new framework package--nothing to check for compatibility
            let new_object = Object::new_system_package(
                modules,
                // note: execution_engine assumes any system package with version
                // OBJECT_START_VERSION is freshly created rather than
                // upgraded
                OBJECT_START_VERSION,
                dependencies,
                // Genesis is fine here, we only use it to calculate an object ref that we can
                // use for all validators to commit to the same bytes in
                // the update
                TransactionDigest::GENESIS_MARKER,
            );

            // Adding a package runs it through the publish path, which enforces this
            // bound and aborts the change epoch transaction if it is exceeded.
            // Refusing the upgrade leaves the network on its current version
            // instead. Upgrades of an existing package are exempt from the
            // bound, so the branch below does not check it.
            let size = new_object
                .data
                .as_opt_package()
                .expect("Created as package")
                .size() as u64;
            let max_size = max_package_size(*id, protocol_config);
            if size > max_size {
                error!("New system package {id} is {size} bytes, over the {max_size} byte limit");
                return None;
            }

            return Some(new_object.object_ref());
        }

        Err(e) => {
            error!("Error loading framework object at {id}: {e:?}");
            return None;
        }
    };

    let cur_ref = cur_object.object_ref();
    let cur_pkg = cur_object
        .data
        .as_opt_package()
        .expect("Framework not package");

    let mut new_object = Object::new_system_package(
        modules,
        // Start at the same version as the current package, and increment if compatibility is
        // successful
        cur_object.version(),
        dependencies,
        cur_object.previous_transaction,
    );

    if cur_ref == new_object.object_ref() {
        return Some(cur_ref);
    }

    let compatibility = Compatibility::framework_upgrade_check();

    let new_pkg = new_object
        .data
        .as_opt_mut_package()
        .expect("Created as package");

    let pool = &mut normalized::RcPool::new();
    let cur_normalized = match cur_pkg.normalize(pool, binary_config, /* include code */ false) {
        Ok(v) => v,
        Err(e) => {
            error!("Could not normalize existing package: {e:?}");
            return None;
        }
    };
    let mut new_normalized = new_pkg
        .normalize(pool, binary_config, /* include code */ false)
        .ok()?;

    for (name, cur_module) in cur_normalized {
        let new_module = new_normalized.remove(&name)?;

        if let Err(e) = compatibility.check(&cur_module, &new_module) {
            error!("Compatibility check failed, for new version of {id}::{name}: {e:?}");
            return None;
        }
    }

    new_pkg
        .increment_version()
        .expect("package version should never overflow");

    Some(new_object.object_ref())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    /// Every system package has to fit `max_move_system_package_size`, the
    /// bound it is held to when it is published for the first time, at genesis
    /// or when it is added at an epoch change. Its bytes are fixed when the
    /// binary is built, so a package that has outgrown the bound is caught
    /// here, rather than by a network that will not start or an epoch change
    /// that aborts.
    #[test]
    fn system_packages_fit_the_size_limit() {
        let protocol_config = ProtocolConfig::get_for_max_version_UNSAFE();
        let mut published: BTreeMap<ObjectId, MovePackage> = BTreeMap::new();

        for package in BuiltInFramework::iter_system_packages() {
            let dependencies: Vec<_> = package
                .dependencies
                .iter()
                .map(|id| &published[id])
                .collect();

            MovePackage::new_initial(&package.modules(), &protocol_config, dependencies)
                .unwrap_or_else(|e| {
                    panic!("system package {} cannot be published: {e}", package.id)
                });

            published.insert(package.id, package.genesis_move_package());
        }
    }
}
