// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub mod cached_data_store;
pub mod iota_data_store;
pub mod linkage_view;

use std::rc::Rc;

use iota_sdk_types::{ObjectId, move_package::MovePackage};
use iota_types::{error::IotaResult, storage::BackingPackageStore};

// A unifying trait that allows us to load move packages that may not be objects
// just yet (e.g., if they were published in the current transaction). Note that
// this needs to load `MovePackage`s and not `MovePackageObject`s.
pub trait PackageStore {
    fn get_package(&self, id: &ObjectId) -> IotaResult<Option<Rc<MovePackage>>>;
}

impl<T: BackingPackageStore> PackageStore for T {
    fn get_package(&self, id: &ObjectId) -> IotaResult<Option<Rc<MovePackage>>> {
        Ok(self
            .get_package_object(id)?
            .map(|x| Rc::new(x.move_package().clone())))
    }
}
