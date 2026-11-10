// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{cell::RefCell, rc::Rc, sync::Arc};

use indexmap::IndexMap;
use iota_sdk_types::{MovePackage, ObjectId};
use iota_types::{
    error::{ExecutionError, IotaError, IotaResult},
    storage::BackingPackageStore,
};
use move_core_types::{
    account_address::AccountAddress,
    resolver::{PackageResolver, SerializedPackage},
};
use move_vm_runtime::{
    shared::types::VersionId, validation::verification::ast::Package as VerifiedPackage,
};

use crate::{data_store::serialized_package, sdk_conv::AccountAddressExt};

/// A `TransactionPackageStore` is a `PackageResolver` that fetches packages from a backing store.
/// It also tracks packages that are being published in the current transaction and allows
/// "loading" of those packages as well.
///
/// It is used to provide package loading (from storage) for the Move VM.
#[allow(clippy::type_complexity)]
pub struct TransactionPackageStore<'a> {
    package_store: &'a dyn BackingPackageStore,

    /// Elements in this are packages that are being published or have been published in the
    /// current transaction.
    /// Elements in this are _not_ safe to evict, unless evicted through `pop_package`.
    new_packages: RefCell<IndexMap<ObjectId, (Rc<MovePackage>, Arc<VerifiedPackage>)>>,

    /// A cache of packages that we've loaded so far. This is used to speed up package loading.
    /// Elements in this are safe to be evicted based on cache decisions.
    package_cache: RefCell<IndexMap<ObjectId, Option<Rc<MovePackage>>>>,
}

impl<'a> TransactionPackageStore<'a> {
    pub fn new(package_store: &'a dyn BackingPackageStore) -> Self {
        Self {
            package_store,
            new_packages: RefCell::new(IndexMap::new()),
            package_cache: RefCell::new(IndexMap::new()),
        }
    }

    /// Push a new package into the new packages. This is used to track packages that are being
    /// published or have been published.
    pub fn push_package(
        &self,
        id: ObjectId,
        package: Rc<MovePackage>,
        verified_package: VerifiedPackage,
    ) -> Result<(), ExecutionError> {
        // Check that the package ID is not already present anywhere.
        debug_assert!(!self.new_packages.borrow().contains_key(&id));

        // Insert the package into the new packages
        // If the package already exists, we will overwrite it and signal an error.
        if self
            .new_packages
            .borrow_mut()
            .insert(id, (package, Arc::new(verified_package)))
            .is_some()
        {
            invariant_violation!(
                "Package with ID {} already exists in the new packages. This should never happen.",
                id
            );
        }

        Ok(())
    }

    /// Rollback a package that was pushed into the new packages. We keep the invariant that:
    /// * You can only pop the most recent package that was pushed.
    /// * The element being popped _must_ exist in the new packages.
    ///
    /// Otherwise this returns an invariant violation.
    pub fn pop_package(&self, id: ObjectId) -> Result<Arc<VerifiedPackage>, ExecutionError> {
        if self
            .new_packages
            .borrow()
            .last()
            .is_none_or(|(pkg_id, _)| *pkg_id != id)
        {
            make_invariant_violation!(
                "Tried to pop package {} from new packages, but new packages was empty or \
                it is not the most recent package inserted. This should never happen.",
                id
            );
        }

        let Some((pkg_id, (_move_pkg, verified_pkg))) = self.new_packages.borrow_mut().pop() else {
            unreachable!(
                "We just checked that new packages is not empty, so this should never happen."
            );
        };
        assert_eq!(
            pkg_id, id,
            "Popped package ID {pkg_id} does not match requested ID {id}. This should never happen as was checked above."
        );

        Ok(verified_pkg)
    }

    /// Fetch a package that is being published in the current transaction, if it exists.
    /// This does not look in the backing store.
    pub fn fetch_new_package(
        &self,
        id: &ObjectId,
    ) -> Option<(Rc<MovePackage>, Arc<VerifiedPackage>)> {
        self.new_packages.borrow().get(id).cloned()
    }

    /// Return all new packages that have been added to this store in the transaction.
    pub fn to_new_packages(&self) -> Vec<MovePackage> {
        self.new_packages
            .borrow()
            .iter()
            .map(|(_, (move_pkg, _))| move_pkg.as_ref().clone())
            .collect()
    }

    /// Fetch a package by its version ID. This will first look in the new packages, and then in
    /// the cache for any packages that have been loaded this transaction, and then in the backing
    /// store. If found, it will be returned as a [`MovePackage`].
    pub fn fetch_move_package(
        &self,
        package_version_id: VersionId,
    ) -> IotaResult<Option<Rc<MovePackage>>> {
        if let Some((move_pkg, _verified_pkg)) =
            self.fetch_new_package(&package_version_id.to_object_id())
        {
            return Ok(Some(move_pkg));
        }

        if let Some(cached_pkg) = self
            .package_cache
            .borrow()
            .get(&package_version_id.to_object_id())
        {
            return Ok(cached_pkg.clone());
        }

        let move_package = self
            .package_store
            .get_package_object(&package_version_id.to_object_id())?
            .map(|pkg| Rc::new(pkg.move_package().clone()));

        self.package_cache
            .borrow_mut()
            .insert(package_version_id.to_object_id(), move_package.clone());

        Ok(move_package)
    }

    /// Fetch a package by its version ID. This will first look in the new packages, and then in
    /// the cache for any packages that have been loaded this transaction, and then in the backing
    /// store. If found, it will be returned as a [`SerializedPackage`].
    fn fetch_package(
        &self,
        package_version_id: VersionId,
    ) -> IotaResult<Option<SerializedPackage>> {
        self.fetch_move_package(package_version_id)
            .and_then(|opt_pkg| {
                opt_pkg
                    .as_ref()
                    .map(|pkg| serialized_package(pkg.as_ref()))
                    .transpose()
            })
    }
}

// Better days have arrived!
impl PackageResolver for TransactionPackageStore<'_> {
    type Error = IotaError;
    fn get_packages_static<const N: usize>(
        &self,
        ids: [AccountAddress; N],
    ) -> Result<[Option<SerializedPackage>; N], Self::Error> {
        // Once https://doc.rust-lang.org/stable/std/primitive.array.html#method.try_map is stable
        // we can use that here.
        let mut packages = [const { None }; N];
        for (i, id) in ids.iter().enumerate() {
            packages[i] = self.fetch_package(*id)?;
        }

        Ok(packages)
    }

    fn get_packages<'a>(
        &self,
        ids: impl ExactSizeIterator<Item = &'a AccountAddress>,
    ) -> Result<Vec<Option<SerializedPackage>>, Self::Error> {
        ids.map(|id| self.fetch_package(*id)).collect()
    }
}
