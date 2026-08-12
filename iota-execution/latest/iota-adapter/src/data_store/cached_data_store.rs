// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use iota_sdk_types::{ObjectId, move_package::MovePackage};
use iota_types::error::IotaResult;

use crate::data_store::PackageStore;

pub struct CachedPackageStore<'state> {
    pub package_store: Box<dyn PackageStore + 'state>,
    pub package_cache: RefCell<BTreeMap<ObjectId, Option<Rc<MovePackage>>>>,
    pub max_cache_size: usize,
}

impl<'state> CachedPackageStore<'state> {
    pub const DEFAULT_MAX_CACHE_SIZE: usize = 200;
    pub fn new(package_store: Box<dyn PackageStore + 'state>) -> Self {
        Self {
            package_store,
            package_cache: RefCell::new(BTreeMap::new()),
            max_cache_size: Self::DEFAULT_MAX_CACHE_SIZE,
        }
    }

    pub fn get_package(&self, id: &ObjectId) -> IotaResult<Option<Rc<MovePackage>>> {
        if let Some(pkg) = self.package_cache.borrow().get(id).cloned() {
            return Ok(pkg);
        }

        if self.package_cache.borrow().len() >= self.max_cache_size {
            self.package_cache.borrow_mut().clear();
        }

        let pkg = self.package_store.get_package(id)?;
        self.package_cache.borrow_mut().insert(*id, pkg.clone());
        Ok(pkg)
    }
}

impl PackageStore for CachedPackageStore<'_> {
    fn get_package(&self, id: &ObjectId) -> IotaResult<Option<Rc<MovePackage>>> {
        self.get_package(id)
    }
}
