// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{rc::Rc, sync::Arc};

use iota_sdk_types::{MovePackage, ObjectId};
use iota_types::error::{ExecutionError, ExecutionErrorKind, IotaResult};
use move_core_types::{identifier::IdentStr, resolver::IntraPackageName};
use move_vm_runtime::{
    cache::move_cache::ResolvedPackageResult, runtime::MoveRuntime,
    validation::verification::ast::Package as VerifiedPackage,
};

use crate::{
    compat::IotaErrorKind,
    data_store::{PackageStore, transaction_package_store::TransactionPackageStore},
    sdk_conv::{AccountAddressExt, ToAccountAddress},
};

/// The `CachedPackageStore` is a `PackageStore` implementation that uses a `MoveRuntime` to
/// fetch and cache packages. It also uses an underlying `TransactionPackageStore` to fetch packages
/// that are not in the cache. This is used to provide package loading (from storage)
/// for the Move VM, while also allowing for packages that are being published in the
/// current transaction to be found.
pub struct CachedPackageStore<'state, 'runtime> {
    /// The Move runtime to use for fetching and caching packages.
    runtime: &'runtime MoveRuntime,

    /// Underlying store to fetch packages from. Any newly published packages in the current
    /// transaction should be in the `new_packages` field of this store.
    pub package_store: TransactionPackageStore<'state>,
}

impl<'state, 'runtime> CachedPackageStore<'state, 'runtime> {
    pub fn new(
        runtime: &'runtime MoveRuntime,
        package_store: TransactionPackageStore<'state>,
    ) -> Self {
        Self {
            runtime,
            package_store,
        }
    }

    /// Get a package by its package ID (i.e., not original ID). This will first look in the new
    /// packages, and then fetch the package from the underlying Move runtime which handles loading
    /// and caching of packages. If the package is not found, None is returned. If there is an error
    /// fetching the package, an error is returned.
    ///
    /// Once the package is fetched it is in the Move runtime cache, and will be found there on
    /// subsequent lookups.
    pub fn get_package(&self, object_id: &ObjectId) -> IotaResult<Option<Arc<VerifiedPackage>>> {
        self.fetch_package(object_id)
    }

    pub fn get_move_package(&self, object_id: &ObjectId) -> IotaResult<Option<Rc<MovePackage>>> {
        self.package_store
            .fetch_move_package(object_id.to_account_address())
    }

    /// Get a package by its package ID (i.e., not original ID). This will first look in the new
    /// packages, and then fetch the package from the underlying Move runtime which handles loading
    /// and caching of packages.
    fn fetch_package(&self, id: &ObjectId) -> IotaResult<Option<Arc<VerifiedPackage>>> {
        // Look for package in new packages first. If we have just published the package we are
        // looking up it may not be in the VM runtime cache yet, and we don't want to add it to the
        // cache either. So if it's in the new packages, we return it directly.
        if let Some((_move_pkg, verified_pkg)) = self.package_store.fetch_new_package(id) {
            return Ok(Some(verified_pkg));
        }

        // load the package via the Move runtime, which will cache it if found.
        match self
            .runtime
            .resolve_and_cache_package(&self.package_store, id.to_account_address())
            .map_err(|e| {
                IotaErrorKind::Execution(
                    ExecutionError::new_with_source(
                        ExecutionErrorKind::VmVerificationOrDeserializationError,
                        e.to_string(),
                    )
                    .to_string(),
                )
            })? {
            ResolvedPackageResult::Found(pkg) => Ok(Some(pkg.verified.clone())),
            ResolvedPackageResult::NotFound => Ok(None),
        }
    }
}

impl PackageStore for CachedPackageStore<'_, '_> {
    type Package = Arc<VerifiedPackage>;

    fn get_package(&self, id: &ObjectId) -> IotaResult<Option<Self::Package>> {
        self.get_package(id)
    }

    fn resolve_type_to_defining_id(
        &self,
        module_address: ObjectId,
        module_name: &IdentStr,
        type_name: &IdentStr,
    ) -> IotaResult<Option<ObjectId>> {
        let Some(pkg) = self.get_package(&module_address)? else {
            return Ok(None);
        };

        Ok(pkg
            .type_origin_table()
            .get(&IntraPackageName {
                module_name: module_name.to_owned(),
                type_name: type_name.to_owned(),
            })
            .map(|id| id.to_object_id()))
    }
}
