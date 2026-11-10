// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub mod cached_package_store;
pub mod transaction_package_store;

use std::{collections::BTreeMap, sync::Arc};

use iota_sdk_types::{MovePackage, ObjectId};
use iota_types::{error::IotaResult, move_package::MovePackageExt};
use move_core_types::{
    identifier::{IdentStr, Identifier},
    resolver::{IntraPackageName, SerializedPackage},
};
use move_vm_runtime::{
    shared::types::{OriginalId, VersionId},
    validation::verification::ast::Package as VerifiedPackage,
};

use crate::{
    compat::IotaErrorKind,
    sdk_conv::{AccountAddressExt, ToAccountAddress},
};

/// The VM-independent package metadata required for linkage analysis.
pub trait PackageMetadata {
    fn version(&self) -> u64;
    fn version_id(&self) -> ObjectId;
    fn original_id(&self) -> ObjectId;
    fn linkage_table(&self) -> BTreeMap<OriginalId, VersionId>;
}

/// Access to package information needed for linkage analysis.
pub trait PackageStore {
    type Package: PackageMetadata;

    fn get_package(&self, id: &ObjectId) -> IotaResult<Option<Self::Package>>;

    fn resolve_type_to_defining_id(
        &self,
        module_address: ObjectId,
        module_name: &IdentStr,
        type_name: &IdentStr,
    ) -> IotaResult<Option<ObjectId>>;
}

/// A package store whose loaded packages have been verified by the Move VM.
///
/// Some move packages that can be loaded through this store may not be objects yet (for example,
/// packages published in the current transaction).
pub type VerifiedPackageStore<'a> = dyn PackageStore<Package = Arc<VerifiedPackage>> + 'a;

impl PackageMetadata for Arc<VerifiedPackage> {
    fn version(&self) -> u64 {
        VerifiedPackage::version(self.as_ref())
    }

    fn version_id(&self) -> ObjectId {
        VerifiedPackage::version_id(self.as_ref()).to_object_id()
    }

    fn original_id(&self) -> ObjectId {
        VerifiedPackage::original_id(self.as_ref()).to_object_id()
    }

    fn linkage_table(&self) -> BTreeMap<OriginalId, VersionId> {
        VerifiedPackage::linkage_table(self.as_ref()).clone()
    }
}

/// Converts an on-chain package into the runtime's loading representation. Upstream this is a
/// method on the package type; IOTA keeps the SDK package type independent of the Move crates.
pub(crate) fn serialized_package(pkg: &MovePackage) -> IotaResult<SerializedPackage> {
    fn identifier(name: &iota_sdk_types::Identifier) -> IotaResult<Identifier> {
        Identifier::new(name.as_str()).map_err(|e| {
            debug_assert!(
                false,
                "Published modules must always have valid identifiers {e}"
            );
            IotaErrorKind::ExecutionInvariantViolation
        })
    }
    let type_origin_table = pkg
        .type_origin_table()
        .iter()
        .map(|ty_origin| {
            Ok((
                IntraPackageName {
                    module_name: identifier(&ty_origin.module_name)?,
                    type_name: identifier(&ty_origin.datatype_name)?,
                },
                ty_origin.package.to_account_address(),
            ))
        })
        .collect::<IotaResult<_>>()?;
    let original_id = pkg.original_package_id();
    Ok(SerializedPackage {
        modules: pkg
            .serialized_module_map()
            .iter()
            .map(|(k, v)| Ok((identifier(k)?, v.clone())))
            .collect::<IotaResult<_>>()?,
        version_id: pkg.id().to_account_address(),
        original_id: original_id.to_account_address(),
        linkage_table: pkg
            .linkage_table()
            .iter()
            .map(|(k, v)| (k.to_account_address(), v.upgraded_id.to_account_address()))
            .chain(std::iter::once((
                original_id.to_account_address(),
                pkg.id().to_account_address(),
            )))
            .collect(),
        type_origin_table,
        version: pkg.version().as_u64(),
    })
}
