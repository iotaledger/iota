// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, rc::Rc};

use iota_sdk_types::ObjectId;
use iota_types::{IOTA_FRAMEWORK_PACKAGE_ID, IOTA_SYSTEM_PACKAGE_ID, MOVE_STDLIB_PACKAGE_ID};
use move_binary_format::binary_config::BinaryConfig;
use move_vm_runtime::shared::types::{OriginalId, VersionId};

use crate::{
    compat::ExecutionErrorTrait,
    data_store::{PackageMetadata, PackageStore},
    static_programmable_transactions::linkage::resolution::{
        ResolutionTable, VersionConstraint, add_and_unify,
    },
};

/// These are the set of native packages in IOTA -- importantly they can be used implicitly by
/// different parts of the system and are not required to be explicitly imported always.
/// Additionally, there is no versioning concerns around these as they are "stable" for a given
/// epoch, and are the special packages that are always available, and updated in-place.
const NATIVE_PACKAGE_IDS: &[ObjectId] = &[
    IOTA_FRAMEWORK_PACKAGE_ID,
    IOTA_SYSTEM_PACKAGE_ID,
    MOVE_STDLIB_PACKAGE_ID,
];

/// Metadata and shared operations for the PTB linkage analysis.
#[derive(Debug)]
pub struct ResolutionConfig_ {
    /// Config to use for the linkage analysis.
    linkage_config: LinkageConfig,
    /// Config to use for the binary analysis (needed for deserialization to determine if a
    /// function is a non-public entry function).
    binary_config: BinaryConfig,
}

#[derive(Debug, Clone)]
pub struct ResolutionConfig(Rc<ResolutionConfig_>);

/// Configuration for the linkage analysis.
#[derive(Debug, Clone)]
pub struct LinkageConfig {
    /// Whether system packages should always be included as a member in the generated linkage.
    /// This is almost always true except for system transactions and genesis transactions.
    pub always_include_system_packages: bool,
}

impl ResolutionConfig {
    pub fn new(linkage_config: LinkageConfig, binary_config: BinaryConfig) -> Self {
        Self(Rc::new(ResolutionConfig_ {
            linkage_config,
            binary_config,
        }))
    }

    pub fn linkage_config(&self) -> &LinkageConfig {
        &self.0.linkage_config
    }

    pub fn binary_config(&self) -> &BinaryConfig {
        &self.0.binary_config
    }

    pub(crate) fn resolution_table_with_native_packages<
        E: ExecutionErrorTrait,
        S: PackageStore + ?Sized,
    >(
        &self,
        store: &S,
    ) -> Result<ResolutionTable, E> {
        let mut resolution_table = ResolutionTable::empty(self.clone());
        if self.0.linkage_config.always_include_system_packages {
            for id in NATIVE_PACKAGE_IDS {
                #[cfg(debug_assertions)]
                {
                    use crate::static_programmable_transactions::linkage::resolution::get_package;
                    let package = get_package(id, store)?;
                    debug_assert_eq!(package.version_id(), *id);
                    debug_assert_eq!(package.original_id(), *id);
                }
                add_and_unify(id, store, &mut resolution_table, VersionConstraint::exact)?;
            }
        }

        Ok(resolution_table)
    }

    pub(crate) fn linkage_table<P: PackageMetadata>(
        &self,
        pkg: &P,
    ) -> BTreeMap<OriginalId, VersionId> {
        pkg.linkage_table()
    }
}

impl LinkageConfig {
    pub fn new(always_include_system_packages: bool) -> Self {
        Self {
            always_include_system_packages,
        }
    }
}
